//! Reads criterion benchmark results from `target/criterion/` JSON files and
//! renders a markdown table similar to criterion-table.
//!
//! # Example
//!
//! ```rust,no_run
//! use criterion_markdown::{Emojis, Renderer, Thresholds};
//!
//! fn main() -> anyhow::Result<()> {
//!     let thresholds = Thresholds::default()
//!         .improvement_ratio(1.1)
//!         .strong_improvement_ratio(1.5)
//!         .regression_ratio(0.95);
//!     let emojis = Emojis::default().strong_improvement("🔥");
//!     let markdown = Renderer::new("target/criterion")
//!         .candidate("new")
//!         .baseline_root("artifacts/criterion")
//!         .baseline("main")
//!         .benchmarks(["group/benchmark/1", "group/benchmark/2"])
//!         .thresholds(thresholds)
//!         .emojis(emojis)
//!         .summary_limit(5)
//!         .title("Benchmark Results")
//!         .collapsible(true)
//!         .render()?;
//!     println!("{markdown}");
//!     Ok(())
//! }
//! ```

use std::path::{Path, PathBuf};

use anyhow::Result;

mod discovery;
mod markdown;
mod model;

/// Ratio thresholds controlling change indicators.
///
/// Ratios are calculated as `baseline time / candidate time` and classified as
/// regression, neutral, improvement, or strong improvement.
#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    improvement_ratio: f64,
    strong_improvement_ratio: f64,
    regression_ratio: f64,
}

impl Thresholds {
    /// Sets the minimum ratio rendered as an improvement. Defaults to `1.1`.
    pub fn improvement_ratio(mut self, ratio: f64) -> Self {
        self.improvement_ratio = ratio;
        self
    }

    /// Sets the minimum ratio rendered as a strong improvement. Defaults to `1.8`.
    pub fn strong_improvement_ratio(mut self, ratio: f64) -> Self {
        self.strong_improvement_ratio = ratio;
        self
    }

    /// Sets the maximum ratio rendered as a regression. Defaults to `0.9`.
    pub fn regression_ratio(mut self, ratio: f64) -> Self {
        self.regression_ratio = ratio;
        self
    }

    fn validate(self) -> Result<()> {
        if !self.improvement_ratio.is_finite() || self.improvement_ratio < 1.0 {
            anyhow::bail!("improvement ratio must be finite and at least 1.0");
        }
        if !self.strong_improvement_ratio.is_finite()
            || self.strong_improvement_ratio < self.improvement_ratio
        {
            anyhow::bail!(
                "strong improvement ratio must be finite and at least the improvement ratio"
            );
        }
        if !self.regression_ratio.is_finite()
            || self.regression_ratio <= 0.0
            || self.regression_ratio > 1.0
        {
            anyhow::bail!("regression ratio must be finite, positive, and at most 1.0");
        }
        Ok(())
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            improvement_ratio: 1.1,
            strong_improvement_ratio: 1.8,
            regression_ratio: 0.9,
        }
    }
}

/// Emojis used to represent benchmark change classifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emojis {
    pub(crate) regression: String,
    pub(crate) stable: String,
    pub(crate) improvement: String,
    pub(crate) strong_improvement: String,
}

impl Emojis {
    /// Sets the emoji rendered for regressions. Defaults to `❌`.
    pub fn regression(mut self, emoji: impl AsRef<str>) -> Self {
        self.regression = emoji.as_ref().to_string();
        self
    }

    /// Sets the emoji rendered for stable results. Defaults to `➖`.
    pub fn stable(mut self, emoji: impl AsRef<str>) -> Self {
        self.stable = emoji.as_ref().to_string();
        self
    }

    /// Sets the emoji rendered for improvements. Defaults to `↗️`.
    pub fn improvement(mut self, emoji: impl AsRef<str>) -> Self {
        self.improvement = emoji.as_ref().to_string();
        self
    }

    /// Sets the emoji rendered for strong improvements. Defaults to `🚀`.
    pub fn strong_improvement(mut self, emoji: impl AsRef<str>) -> Self {
        self.strong_improvement = emoji.as_ref().to_string();
        self
    }
}

impl Default for Emojis {
    fn default() -> Self {
        Self {
            regression: "❌".to_string(),
            stable: "➖".to_string(),
            improvement: "↗️".to_string(),
            strong_improvement: "🚀".to_string(),
        }
    }
}

/// Builder for loading Criterion results and rendering them as markdown.
#[derive(Debug, Clone)]
pub struct Renderer {
    criterion_dir: PathBuf,
    baseline_root: PathBuf,
    candidate: String,
    baseline: Option<String>,
    included_entries: Vec<String>,
    title: String,
    collapsible: bool,
    thresholds: Thresholds,
    emojis: Emojis,
    summary_limit: usize,
}

impl Renderer {
    /// Creates a renderer for a Criterion output directory.
    pub fn new(criterion_dir: impl AsRef<Path>) -> Self {
        let criterion_dir = criterion_dir.as_ref().to_path_buf();
        Self {
            baseline_root: criterion_dir.clone(),
            criterion_dir,
            candidate: "new".to_string(),
            baseline: None,
            included_entries: Vec::new(),
            title: "Benchmarks".to_string(),
            collapsible: false,
            thresholds: Thresholds::default(),
            emojis: Emojis::default(),
            summary_limit: 3,
        }
    }

    /// Selects the dataset to render. Defaults to `new`.
    pub fn candidate(mut self, candidate: impl AsRef<str>) -> Self {
        self.candidate = candidate.as_ref().to_string();
        self
    }

    /// Selects the baseline used to compute changes.
    ///
    /// If not selected, the renderer uses Criterion's default `base` dataset
    /// when it exists and otherwise omits comparison information. Rendering
    /// fails if an explicitly selected baseline cannot be found.
    pub fn baseline(mut self, baseline: impl AsRef<str>) -> Self {
        self.baseline = Some(baseline.as_ref().to_string());
        self
    }

    /// Selects the Criterion output root containing the baseline.
    ///
    /// Defaults to the candidate's Criterion output directory.
    pub fn baseline_root(mut self, baseline_root: impl AsRef<Path>) -> Self {
        self.baseline_root = baseline_root.as_ref().to_path_buf();
        self
    }

    /// Selects a benchmark by its full id.
    ///
    /// If no benchmarks are selected explicitly, all benchmarks are rendered.
    pub fn benchmark(mut self, entry: impl AsRef<str>) -> Self {
        self.included_entries.push(entry.as_ref().to_string());
        self
    }

    /// Selects benchmarks by their full ids.
    ///
    /// Repeated calls are additive. If no benchmarks are selected explicitly,
    /// all benchmarks are rendered.
    pub fn benchmarks(mut self, entries: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.included_entries
            .extend(entries.into_iter().map(|entry| entry.as_ref().to_string()));
        self
    }

    /// Sets the report title. Defaults to `Benchmarks`.
    pub fn title(mut self, title: impl AsRef<str>) -> Self {
        self.title = title.as_ref().to_string();
        self
    }

    /// Controls whether the output is wrapped in a `<details>` element.
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Configures the ratios used to select change indicators.
    pub fn thresholds(mut self, thresholds: Thresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Configures the emojis used for benchmark change classifications.
    pub fn emojis(mut self, emojis: Emojis) -> Self {
        self.emojis = emojis;
        self
    }

    /// Sets the maximum number of improvements and regressions shown in the summary.
    /// Defaults to `3`. A limit of `0` omits the summary.
    pub fn summary_limit(mut self, limit: usize) -> Self {
        self.summary_limit = limit;
        self
    }

    /// Loads the configured results and renders them as markdown.
    pub fn render(&self) -> Result<String> {
        self.thresholds.validate()?;
        let mut entries = discovery::discover_benchmarks(
            &self.criterion_dir,
            &self.candidate,
            &self.baseline_root,
            self.baseline.as_deref(),
        )?;
        if !self.included_entries.is_empty() {
            entries.retain(|entry| self.included_entries.contains(&entry.full_id));
        }
        if entries.is_empty() {
            anyhow::bail!(
                "No benchmark results found in {}",
                self.criterion_dir.display()
            );
        }
        let summary =
            markdown::compute_summary(&entries, self.thresholds, &self.emojis, self.summary_limit);
        let body = markdown::format_table(
            &entries,
            &self.title,
            self.collapsible,
            self.thresholds,
            &self.emojis,
            &summary,
        );
        if !self.collapsible {
            return Ok(body);
        }
        let headline = summary
            .headline(&self.emojis)
            .map(|headline| format!(" ({headline})"))
            .unwrap_or_default();
        Ok(format!(
            "<details>\n<summary>{}{headline}</summary>\n\n{body}\n</details>\n",
            self.title
        ))
    }
}
