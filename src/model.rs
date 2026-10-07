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

impl Estimates {
    /// The estimate criterion reports as typical.
    ///
    /// Linear sampling fits a regression whose slope is weighted towards the
    /// longest samples, so warm-up in the short ones barely moves it. Flat
    /// sampling fits no regression and leaves the mean.
    pub(crate) fn typical(&self) -> &Estimate {
        self.slope.as_ref().unwrap_or(&self.mean)
    }
}

/// A single statistical estimate with confidence interval.
#[derive(Deserialize)]
pub(crate) struct Estimate {
    pub(crate) point_estimate: f64,
    pub(crate) confidence_interval: ConfidenceInterval,
}

/// The bounds criterion bootstrapped for an estimate.
#[derive(Deserialize)]
pub(crate) struct ConfidenceInterval {
    pub(crate) lower_bound: f64,
    pub(crate) upper_bound: f64,
}

/// Parsed change information for a benchmark.
pub(crate) struct ChangeInfo {
    /// Relative change as a fraction (e.g., 0.05 = +5%, -0.02 = -2%).
    pub(crate) point_estimate: f64,
    /// Smallest ratio of current to baseline both intervals allow.
    pub(crate) lower_ratio: f64,
    /// Largest ratio of current to baseline both intervals allow.
    pub(crate) upper_ratio: f64,
}

impl ChangeInfo {
    pub(crate) fn from_estimates(current: &Estimates, baseline: &Estimates) -> Self {
        // Compare like with like. A benchmark whose sampling mode changed
        // between the two runs shares only the mean.
        let (current, baseline) = match (&current.slope, &baseline.slope) {
            (Some(current), Some(baseline)) => (current, baseline),
            _ => (&current.mean, &baseline.mean),
        };

        Self {
            point_estimate: current.point_estimate / baseline.point_estimate - 1.0,
            lower_ratio: current.confidence_interval.lower_bound
                / baseline.confidence_interval.upper_bound,
            upper_ratio: current.confidence_interval.upper_bound
                / baseline.confidence_interval.lower_bound,
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
    use super::{ChangeInfo, Estimates};

    /// Estimates where the slope and the mean disagree, taken from
    /// `slot_pool/alloc_dealloc_1500` on a run whose first 32 samples were
    /// still warming up. The mean reads 1.56x slower, the slope 0.93x.
    const WARMED_UP: &str = r#"{
        "mean": {
            "point_estimate": 12.952912949016083,
            "confidence_interval": { "lower_bound": 11.260847001464509,
                                     "upper_bound": 14.722386453354728 }
        },
        "slope": {
            "point_estimate": 7.707953797660542,
            "confidence_interval": { "lower_bound": 7.360335814242837,
                                     "upper_bound": 8.182146986705531 }
        }
    }"#;

    const STEADY: &str = r#"{
        "mean": {
            "point_estimate": 8.299592350746269,
            "confidence_interval": { "lower_bound": 8.264150943396226,
                                     "upper_bound": 8.340989399293286 }
        },
        "slope": {
            "point_estimate": 8.333333333333334,
            "confidence_interval": { "lower_bound": 8.291666666666666,
                                     "upper_bound": 8.375000000000000 }
        }
    }"#;

    fn parse(json: &str) -> Estimates {
        serde_json::from_str(json).expect("parse criterion estimates")
    }

    /// The reported time comes from the slope, so the change must too.
    /// Comparing means here would call a faster benchmark 1.56x slower.
    #[test]
    fn change_follows_the_estimate_the_report_displays() {
        let current = parse(WARMED_UP);
        let baseline = parse(STEADY);

        let change = ChangeInfo::from_estimates(&current, &baseline);
        let ratio = 1.0 + change.point_estimate;

        assert!(
            (ratio - 0.924_954_455_7).abs() < 1e-9,
            "expected the slope ratio, got {ratio}"
        );
    }

    /// Flat sampling records no slope, leaving the mean as the only estimate.
    #[test]
    fn change_falls_back_to_the_mean_without_a_slope() {
        let flat = r#"{
            "mean": {
                "point_estimate": 2.0,
                "confidence_interval": { "lower_bound": 1.9, "upper_bound": 2.1 }
            },
            "slope": null
        }"#;
        let baseline = r#"{
            "mean": {
                "point_estimate": 1.0,
                "confidence_interval": { "lower_bound": 0.95, "upper_bound": 1.05 }
            },
            "slope": null
        }"#;

        let change = ChangeInfo::from_estimates(&parse(flat), &parse(baseline));

        assert!((change.point_estimate - 1.0).abs() < 1e-9);
    }

    /// One side losing its slope leaves the mean as the only shared estimate.
    #[test]
    fn change_compares_like_with_like_when_sampling_mode_changes() {
        let baseline = r#"{
            "mean": {
                "point_estimate": 8.299592350746269,
                "confidence_interval": { "lower_bound": 8.26, "upper_bound": 8.34 }
            },
            "slope": null
        }"#;

        let change = ChangeInfo::from_estimates(&parse(WARMED_UP), &parse(baseline));
        let ratio = 1.0 + change.point_estimate;

        // Both means, rather than this run's slope against that run's mean.
        assert!((ratio - 1.560_668_572_8).abs() < 1e-9);
    }

    #[test]
    fn change_carries_the_ratio_bounds_of_both_intervals() {
        let change = ChangeInfo::from_estimates(&parse(WARMED_UP), &parse(STEADY));

        assert!((change.lower_ratio - 7.360335814242837 / 8.375).abs() < 1e-9);
        assert!((change.upper_ratio - 8.182146986705531 / 8.291666666666666).abs() < 1e-9);
    }
}
