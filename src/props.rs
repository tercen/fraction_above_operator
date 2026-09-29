//! Operator properties. The threshold itself is not a property: it is the **second row factor**
//! of the projection (marker, then its threshold), so a threshold table joined on the marker
//! feeds this step directly — the pattern `asinh`'s manual mode uses for cofactors.
use anyhow::Result;
use tercen_rs::PropertyReader;
use tercen_rs::context::ContextBase;

#[derive(Debug, Clone)]
pub struct Settings {
    /// Cells with fewer values than this get NaN as their fraction (the count is still written).
    pub min_values: usize,
    /// Count values equal to the threshold as above it.
    pub inclusive: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            min_values: 1,
            inclusive: false,
        }
    }
}

pub fn read(ctx: &ContextBase) -> Result<Settings> {
    let pr = PropertyReader::from_operator_settings(ctx.operator_settings());
    let d = Settings::default();
    let raw = pr.get_string("min_values", &d.min_values.to_string());
    let min_values = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| anyhow::anyhow!("property 'min_values' is not a number: {raw:?}"))?
        .max(1.0) as usize;
    let inclusive = pr.get_string("inclusive", "false").trim().to_lowercase() == "true";
    Ok(Settings {
        min_values,
        inclusive,
    })
}
