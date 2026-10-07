# fraction_above_operator

Rust Tercen operator; skeleton copied from `flowsom_rust_operator` (context, input streaming,
TSON writer, upload, progress, pagecache). Operator logic: 
`src/lib.rs::execute` (stream, count per cell, write). Result is one table keyed by `.ri`, so the
platform test lists **two** expected files (result, row table).

Dev loop: `dev/` scripts as in read_fcs; `target/release/dev` takes `WORKFLOW_ID` / `STEP_ID`
env vars. Fixtures for `tests/test.json` come from a Studio run, exported untouched (stored row
order), with `.schema` sidecars for the int32 columns (`cells_fitted`, `em_iterations`, `.ri`).
