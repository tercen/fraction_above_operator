# fraction_above_operator

Per crosstab cell, the fraction of values above the row's threshold. The threshold is the
**second row factor**: put the marker on rows, then its threshold (from a joined threshold table,
for instance `gmm_threshold_rust_operator`'s output), the groups on columns, the value on y.

| | |
|---|---|
| rows | marker, then threshold |
| columns | the groups: patient × cluster, sample, ... |
| y | the value |
| output | per cell with values: `fraction`, `pct` (= 100 × fraction), `n_above`, `n`; joined on row and column |
| image | `ghcr.io/tercen/fraction_above_operator` |

| property | default | meaning |
|---|---|---|
| `min_values` | 1 | Cells with fewer values get NaN as fraction and pct; counts are still written. `state_analysis.py` uses 5 per patient × cluster. |
| `inclusive` | false | Count values equal to the threshold as above. |

One streaming pass over the crosstab: memory is the number of crosstab cells, not values.
