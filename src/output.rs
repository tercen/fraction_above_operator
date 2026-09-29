//! The result: one row per **row** of the crosstab — one per marker — carrying the fitted
//! mixture and the threshold derived from it, keyed by `.ri`. One table, one relation.
use std::io::Write;

use anyhow::{Result, anyhow};

use crate::tson::TsonWriter;

pub struct ColSpec<'a> {
    pub name: &'a str,
    pub ty: &'a str, // "double" | "int32" | "string"
}

/// `sprintf("c%0Nd", i)` — the width comes from the largest label, as the R operator does it.
pub fn label(i: usize, width: usize) -> String {
    format!("c{i:0width$}")
}

/// Width the R operator uses: the digits of the largest value present.
pub fn label_width(max: usize) -> usize {
    max.max(1).to_string().len()
}

pub fn write_header<W: Write>(
    w: &mut TsonWriter<W>,
    table_name: &str,
    n_rows: usize,
    cols: &[ColSpec],
    n_tables: usize,
) -> Result<()> {
    w.map(3)?;
    w.key("kind")?;
    w.str("OperatorResult")?;
    w.key("tables")?;
    w.list(n_tables)?;

    w.map(4)?;
    w.key("kind")?;
    w.str("Table")?;
    w.key("nRows")?;
    w.i32(i32::try_from(n_rows).map_err(|_| {
        anyhow!(
            "the result would have {n_rows} rows, more than a Tercen table can hold (i32::MAX). \
             Project fewer cells, or split the step."
        )
    })?)?;
    w.key("properties")?;
    w.map(4)?;
    w.key("kind")?;
    w.str("TableProperties")?;
    w.key("name")?;
    w.str(table_name)?;
    w.key("sortOrder")?;
    w.list(0)?;
    w.key("ascending")?;
    w.bool(false)?;
    w.key("columns")?;
    w.list(cols.len())?;
    Ok(())
}

pub fn write_column_header<W: Write>(
    w: &mut TsonWriter<W>,
    c: &ColSpec,
    n_rows: usize,
) -> Result<()> {
    w.map(6)?;
    w.key("kind")?;
    w.str("Column")?;
    w.key("name")?;
    w.str(c.name)?;
    w.key("type")?;
    w.str(c.ty)?;
    w.key("nRows")?;
    w.i32(n_rows as i32)?;
    w.key("size")?;
    w.i32(n_rows as i32)?;
    w.key("values")?;
    Ok(())
}

pub enum Col<'a> {
    F64(&'a str, Vec<f64>),
    I32(&'a str, Vec<i32>),
    Str(&'a str, Vec<String>),
}

/// A table keyed by `.ri`: one row per crosstab row, in row order.
pub fn write_row_table<W: Write>(
    w: &mut TsonWriter<W>,
    table_name: &str,
    n_rows: usize,
    cols: &[Col],
) -> Result<()> {
    let specs: Vec<ColSpec> = cols
        .iter()
        .map(|c| match c {
            Col::F64(n, _) => ColSpec {
                name: n,
                ty: "double",
            },
            Col::I32(n, _) => ColSpec {
                name: n,
                ty: "int32",
            },
            Col::Str(n, _) => ColSpec {
                name: n,
                ty: "string",
            },
        })
        .chain(std::iter::once(ColSpec {
            name: ".ri",
            ty: "int32",
        }))
        .collect();
    write_header(w, table_name, n_rows, &specs, 1)?;
    for (c, spec) in cols.iter().zip(&specs) {
        write_column_header(w, spec, n_rows)?;
        match c {
            Col::F64(_, v) => w.f64_list(v)?,
            Col::I32(_, v) => w.i32_list(v)?,
            Col::Str(_, v) => w.str_list(v)?,
        }
    }
    write_column_header(w, specs.last().unwrap(), n_rows)?;
    w.i32_list(&(0..n_rows as i32).collect::<Vec<_>>())?;
    Ok(())
}

/// A table keyed by `(.ri, .ci)`: one row per crosstab cell that has values.
pub fn write_cell_table<W: Write>(
    w: &mut TsonWriter<W>,
    table_name: &str,
    cols: &[Col],
    ri: &[i32],
    ci: &[i32],
) -> Result<()> {
    let n_rows = ri.len();
    let specs: Vec<ColSpec> = cols
        .iter()
        .map(|c| match c {
            Col::F64(n, _) => ColSpec {
                name: n,
                ty: "double",
            },
            Col::I32(n, _) => ColSpec {
                name: n,
                ty: "int32",
            },
            Col::Str(n, _) => ColSpec {
                name: n,
                ty: "string",
            },
        })
        .chain([
            ColSpec {
                name: ".ri",
                ty: "int32",
            },
            ColSpec {
                name: ".ci",
                ty: "int32",
            },
        ])
        .collect();
    write_header(w, table_name, n_rows, &specs, 1)?;
    for (c, spec) in cols.iter().zip(&specs) {
        write_column_header(w, spec, n_rows)?;
        match c {
            Col::F64(_, v) => w.f64_list(v)?,
            Col::I32(_, v) => w.i32_list(v)?,
            Col::Str(_, v) => w.str_list(v)?,
        }
    }
    write_column_header(w, &specs[cols.len()], n_rows)?;
    w.i32_list(ri)?;
    write_column_header(w, &specs[cols.len() + 1], n_rows)?;
    w.i32_list(ci)?;
    Ok(())
}

/// Close the result. One relation, so no joins to declare.
pub fn write_footer<W: Write>(w: &mut TsonWriter<W>) -> Result<()> {
    w.key("joinOperators")?;
    w.list(0)?;
    w.flush()?;
    Ok(())
}
