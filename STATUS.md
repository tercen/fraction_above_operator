# fraction_above_rust_operator — status, 2026-09-29

0.1.0: streaming counter per crosstab cell; threshold from the second row factor. Built for the run-9-on-Tercen workflow (PLAN.md M3).
table). Gathers the whole crosstab (4 bytes a value), so cohort scale needs the same spill the
other gathering operators need; at 5,000 events per file on the 93-file panel that is 8 M values.
Written for a spectral-flow state-analysis workflow on Tercen.

0.1.1 (2026-09-30): memory model for a streaming operator — counters per crosstab cell only, so a near-constant booking (2 B/value for the streamed pages the cgroup counts + 300 MB), the shape asinh uses. Was FlowSOM's gather model. No code change.
