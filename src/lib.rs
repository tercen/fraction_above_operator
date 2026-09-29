//! fraction_above_operator — per crosstab cell, the fraction of values above the row's threshold.
//!
//! Rows are markers with their threshold as the **second row factor**; columns are the groups
//! (patient × cluster, say); y is the value. One streaming pass, counters per cell: nothing is
//! gathered, so the memory is the number of crosstab cells, not the number of values.
//!
//! Rows are channels, columns are cells, y is the value. The result is one row per cell with
//! its SOM node and its metacluster, plus a table describing the map.
//!
//! **The shape of the problem is a transpose.** The crosstab arrives as scattered
//! `(.ri, .ci, .y)` triples and a map needs each cell's whole vector across channels. There is
//! no order to rely on — R's own client scatters by index rather than assuming one — so the
//! operator gathers the matrix itself, `n_cells × n_channels` of `f32`, and the memory model
//! declares that cost rather than hiding it. `f32` because the R pipeline is `f32` anyway
//! (flowCore stores expressions as 32-bit floats), so the second half of the mantissa was never
//! real.
//!
//! One pass to gather, one map, one assignment, one write.
pub mod context;
pub mod input;
pub mod output;
pub mod pagecache;
pub mod progress;
pub mod props;
pub mod tson;
pub mod upload;

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use tercen_rs::context::ContextBase;
use tercen_rs::{DevContext, TercenClient};

use progress::Reporter;
use tson::TsonWriter;

const CHUNK: usize = 1_000_000;

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

pub fn require_env(name: &str) -> Result<String> {
    std::env::var(name).map_err(|_| anyhow!("{name} is not set"))
}

pub async fn run(task_id: &str) -> Result<()> {
    tracing::info!("gmm_threshold_operator starting (task_id={task_id})");
    let client = build_client().await?;
    let ctx = context::from_task_id(client, task_id).await?;
    execute(
        &ctx,
        Mode::Production {
            task_id: task_id.to_string(),
        },
    )
    .await
}

pub async fn run_dev(workflow_id: &str, step_id: &str) -> Result<()> {
    tracing::info!("gmm_threshold_operator starting in dev mode ({workflow_id} / {step_id})");
    let client = build_client().await?;
    let ctx = DevContext::from_workflow_step(client, workflow_id, step_id)
        .await
        .map_err(|e| anyhow!("load workflow {workflow_id} / step {step_id}: {e}"))?;
    execute(
        &ctx,
        Mode::Dev {
            workflow_id: workflow_id.to_string(),
            step_id: step_id.to_string(),
        },
    )
    .await
}

enum Mode {
    Production {
        task_id: String,
    },
    Dev {
        workflow_id: String,
        step_id: String,
    },
}

async fn build_client() -> Result<Arc<TercenClient>> {
    let client = TercenClient::from_env()
        .await
        .map_err(|e| anyhow!("connect to Tercen: {e}"))?;
    tracing::info!("connected to Tercen");
    Ok(Arc::new(client))
}

async fn execute(ctx: &ContextBase, mode: Mode) -> Result<()> {
    let t_start = Instant::now();
    tracing::info!(
        workflow = ctx.workflow_id(),
        step = ctx.step_id(),
        namespace = ctx.namespace(),
        "context loaded"
    );
    let rep = match &mode {
        Mode::Production { task_id } => Reporter::spawn(Arc::clone(ctx.client()), task_id.clone()),
        Mode::Dev { .. } => Reporter::silent(),
    };
    let s = props::read(ctx)?;
    tracing::info!(?s, "properties");

    let n_values = input::cell_count(ctx).await?;
    let markers = input::row_labels(ctx).await?;
    let p = markers.len();
    if p == 0 {
        anyhow::bail!("the projection has no rows: put the markers on rows");
    }
    let thresholds = input::row_thresholds(ctx).await?;
    if thresholds.len() != p {
        anyhow::bail!("{} thresholds for {p} rows", thresholds.len());
    }
    let n_groups = input::column_count(ctx).await?;
    tracing::info!(n_values, rows = p, columns = n_groups, "projection");

    rep.at(0, "Reading the crosstab");
    let mut n = vec![0u32; p * n_groups];
    let mut above = vec![0u32; p * n_groups];
    let mut seen = 0usize;
    let mut out_of_range = 0usize;
    input::for_each_chunk(ctx, &[".ri", ".ci", ".y"], n_values, CHUNK, |c| {
        for k in 0..c.y.len() {
            let (ri, ci) = (c.ri[k] as usize, c.ci[k] as usize);
            if ri >= p || ci >= n_groups {
                out_of_range += 1;
                continue;
            }
            let v = c.y[k];
            if v.is_nan() {
                continue;
            }
            let idx = ci + ri * n_groups;
            n[idx] += 1;
            let t = thresholds[ri];
            if v > t || (s.inclusive && v == t) {
                above[idx] += 1;
            }
        }
        seen += c.len();
        rep.at(
            progress::band(progress::READ, seen, n_values.max(1)),
            format!("Read {seen} of {n_values}"),
        );
        Ok(())
    })
    .await?;
    if out_of_range > 0 {
        anyhow::bail!(
            "{out_of_range} values fell outside the {n_groups} x {p} crosstab the schema described"
        );
    }
    let filled = n.iter().filter(|v| **v > 0).count();
    rep.info(format!(
        "fraction above threshold: {p} markers x {n_groups} groups, {filled} cells with values, \
         min_values {}",
        s.min_values
    ));

    rep.at(progress::WRITE.0, "Writing the result");
    let work_root = std::env::temp_dir().join(format!(
        "fraction_above_op_{}_{}",
        ctx.workflow_id(),
        ctx.step_id()
    ));
    std::fs::create_dir_all(&work_root)
        .with_context(|| format!("create {}", work_root.display()))?;
    let _guard = TempDirGuard(work_root.clone());
    let result_path = work_root.join("result.tson");
    {
        let f = std::fs::File::create(&result_path)
            .with_context(|| format!("create {}", result_path.display()))?;
        let w = std::io::BufWriter::with_capacity(4 << 20, pagecache::Releasing::new(f, 256 << 20));
        let mut w = TsonWriter::new(w)?;
        let ns = ctx.namespace();
        // Only cells that received values are written: an empty crosstab cell has no fraction.
        let mut ri_col = Vec::with_capacity(filled);
        let mut ci_col = Vec::with_capacity(filled);
        let mut frac = Vec::with_capacity(filled);
        let mut n_col = Vec::with_capacity(filled);
        let mut ab_col = Vec::with_capacity(filled);
        for ri in 0..p {
            for ci in 0..n_groups {
                let idx = ci + ri * n_groups;
                if n[idx] == 0 {
                    continue;
                }
                ri_col.push(ri as i32);
                ci_col.push(ci as i32);
                n_col.push(n[idx] as i32);
                ab_col.push(above[idx] as i32);
                frac.push(if (n[idx] as usize) >= s.min_values {
                    above[idx] as f64 / n[idx] as f64
                } else {
                    f64::NAN
                });
            }
        }
        let cols = [
            output::Col::F64(&format!("{ns}.fraction"), frac),
            output::Col::F64(
                &format!("{ns}.pct"),
                ab_col
                    .iter()
                    .zip(&n_col)
                    .map(|(a, n)| {
                        if (*n as usize) >= s.min_values {
                            100.0 * *a as f64 / *n as f64
                        } else {
                            f64::NAN
                        }
                    })
                    .collect(),
            ),
            output::Col::I32(&format!("{ns}.n_above"), ab_col),
            output::Col::I32(&format!("{ns}.n"), n_col),
        ];
        output::write_cell_table(&mut w, &table_name(ctx), &cols, &ri_col, &ci_col)?;
        output::write_footer(&mut w)?;
    }
    let bytes = std::fs::metadata(&result_path)?.len();
    tracing::info!(bytes, "result written");
    pagecache::release_path(&result_path);

    rep.at(progress::UPLOAD.0, "Uploading the result");
    match mode {
        Mode::Production { task_id } => {
            upload::save_production(ctx, &task_id, &result_path, &rep).await?
        }
        Mode::Dev {
            workflow_id,
            step_id,
        } => {
            let saved = upload::save_dev(ctx, &workflow_id, &step_id, &result_path).await?;
            tracing::info!(task_id = saved.task_id, "dev result saved");
        }
    }
    rep.at(100, "Done");
    tracing::info!(
        total_secs = format!("{:.1}", t_start.elapsed().as_secs_f64()),
        peak_rss_kb = peak_rss_kb().unwrap_or(0),
        "done"
    );
    Ok(())
}

struct TempDirGuard(std::path::PathBuf);
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn peak_rss_kb() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
}

fn table_name(ctx: &ContextBase) -> String {
    format!("{}_{}", ctx.step_id(), ctx.qt_hash())
}
