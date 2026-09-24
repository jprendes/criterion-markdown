use serde::Deserialize;

/// Metadata from a criterion `benchmark.json` file.
#[derive(Deserialize)]
pub(crate) struct BenchmarkMeta {
    pub(crate) group_id: String,
    pub(crate) function_id: Option<String>,
    pub(crate) value_str: Option<String>,
    pub(crate) throughput: Option<Throughput>,
    pub(crate) full_id: String,
}

/// Throughput specification from `benchmark.json`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub(crate) enum Throughput {
    Bytes(u64),
    Elements(u64),
}

/// Statistical estimates from a criterion `estimates.json` file.
#[derive(Deserialize)]
pub(crate) struct Estimates {
    pub(crate) slope: Option<Estimate>,
    pub(crate) mean: Estimate,
}

/// A single statistical estimate with confidence interval.
#[derive(Deserialize)]
pub(crate) struct Estimate {
    pub(crate) point_estimate: f64,
}

/// Parsed change information for a benchmark.
pub(crate) struct ChangeInfo {
    /// Relative change as a fraction (e.g., 0.05 = +5%, -0.02 = -2%).
    pub(crate) point_estimate: f64,
}

impl ChangeInfo {
    pub(crate) fn from_estimates(current: &Estimates, baseline: &Estimates) -> Self {
        Self {
            point_estimate: current.mean.point_estimate / baseline.mean.point_estimate - 1.0,
        }
    }
}

/// A single benchmark entry with its metadata and timing.
pub(crate) struct BenchEntry {
    pub(crate) full_id: String,
    pub(crate) group_id: String,
    pub(crate) function_id: String,
    pub(crate) value_str: Option<String>,
    pub(crate) estimate_ns: f64,
    #[allow(dead_code)]
    pub(crate) throughput: Option<Throughput>,
    /// Change vs the selected baseline, if available.
    pub(crate) change: Option<ChangeInfo>,
}

impl BenchEntry {
    /// Returns the column label for this benchmark (the function name).
    ///
    /// If `value_str` is set, the full `function_id` is the column.
    /// Otherwise, if `function_id` contains "/", the part before the last "/" is the column.
    pub(crate) fn column(&self) -> &str {
        if self.value_str.is_some() {
            return &self.function_id;
        }
        match self.function_id.rfind('/') {
            Some(idx) => &self.function_id[..idx],
            None => &self.function_id,
        }
    }

    /// Returns the row label for this benchmark (the parameter/value).
    ///
    /// Uses `value_str` if set, otherwise the part after the last "/" in `function_id`.
    pub(crate) fn row(&self) -> Option<&str> {
        if let Some(ref v) = self.value_str {
            return Some(v.as_str());
        }
        self.function_id
            .rfind('/')
            .map(|idx| &self.function_id[idx + 1..])
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{ChangeInfo, Estimate, Estimates};

    #[derive(Deserialize)]
    struct CriterionChangeEstimates {
        mean: Estimate,
    }

    #[test]
    fn computed_change_exactly_matches_criterion_point_estimate() {
        // Captured from the example benchmark after running Criterion with
        // `--save-baseline main`, followed by `--baseline main`.
        let current: Estimates = serde_json::from_str(
            r#"{
                "mean": { "point_estimate": 0.4963816178911267 },
                "slope": null
            }"#,
        )
        .expect("parse current Criterion estimates");
        let baseline: Estimates = serde_json::from_str(
            r#"{
                "mean": { "point_estimate": 0.499565914117225 },
                "slope": null
            }"#,
        )
        .expect("parse baseline Criterion estimates");
        let criterion_change: CriterionChangeEstimates = serde_json::from_str(
            r#"{
                "mean": { "point_estimate": -0.006374126288670401 }
            }"#,
        )
        .expect("parse Criterion change estimates");

        let computed = ChangeInfo::from_estimates(&current, &baseline);

        assert_eq!(
            computed.point_estimate,
            criterion_change.mean.point_estimate
        );
    }
}
