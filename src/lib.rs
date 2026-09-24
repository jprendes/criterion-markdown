//! Reads criterion benchmark results from `target/criterion/` JSON files and
//! renders a markdown table similar to criterion-table.
//!
//! # Example
//!
//! ```rust,no_run
//! use criterion_markdown::{ChangeThresholds, Renderer};
//!
//! fn main() -> anyhow::Result<()> {
//!     let thresholds = ChangeThresholds::default()
//!         .improvement_ratio(1.1)
//!         .strong_improvement_ratio(1.5)
//!         .regression_ratio(0.95);
//!     let markdown = Renderer::new("target/criterion")
//!         .candidate("new")
//!         .baseline_root("artifacts/criterion")
//!         .baseline("main")
//!         .benchmarks(["group/benchmark/1", "group/benchmark/2"])
//!         .change_thresholds(thresholds)
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
pub struct ChangeThresholds {
    improvement_ratio: f64,
    strong_improvement_ratio: f64,
    regression_ratio: f64,
}

impl ChangeThresholds {
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

impl Default for ChangeThresholds {
    fn default() -> Self {
        Self {
            improvement_ratio: 1.1,
            strong_improvement_ratio: 1.8,
            regression_ratio: 0.9,
        }
    }
}

/// Options for controlling the rendered markdown output.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// The report title. Defaults to `Benchmarks`.
    pub title: String,
    /// Whether to wrap the output in a `<details>` element.
    pub collapsible: bool,
    /// The Criterion baseline directory to compare against. Defaults to `base`.
    ///
    /// Changes are computed at render time from this baseline's mean estimate.
    pub baseline: String,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            title: "Benchmarks".to_string(),
            collapsible: false,
            baseline: "base".to_string(),
        }
    }
}

/// Builder for loading Criterion results and rendering them as markdown.
#[derive(Debug, Clone)]
pub struct Renderer {
    criterion_dir: PathBuf,
    baseline_root: PathBuf,
    candidate: String,
    baseline: String,
    included_entries: Vec<String>,
    title: String,
    collapsible: bool,
    change_thresholds: ChangeThresholds,
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
            baseline: "base".to_string(),
            included_entries: Vec::new(),
            title: "Benchmarks".to_string(),
            collapsible: false,
            change_thresholds: ChangeThresholds::default(),
            summary_limit: 3,
        }
    }

    /// Selects the dataset to render. Defaults to `new`.
    pub fn candidate(mut self, candidate: impl AsRef<str>) -> Self {
        self.candidate = candidate.as_ref().to_string();
        self
    }

    /// Selects the baseline used to compute changes. Defaults to `base`.
    pub fn baseline(mut self, baseline: impl AsRef<str>) -> Self {
        self.baseline = baseline.as_ref().to_string();
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
    pub fn change_thresholds(mut self, thresholds: ChangeThresholds) -> Self {
        self.change_thresholds = thresholds;
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
        self.change_thresholds.validate()?;
        let mut entries = discovery::discover_benchmarks(
            &self.criterion_dir,
            &self.candidate,
            &self.baseline_root,
            &self.baseline,
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
            markdown::compute_summary(&entries, self.change_thresholds, self.summary_limit);
        let body = markdown::format_table(
            &entries,
            &self.title,
            self.collapsible,
            self.change_thresholds,
            &summary,
        );
        if !self.collapsible {
            return Ok(body);
        }
        let headline = summary
            .headline()
            .map(|headline| format!(" ({headline})"))
            .unwrap_or_default();
        Ok(format!(
            "<details>\n<summary>{}{headline}</summary>\n\n{body}\n</details>\n",
            self.title
        ))
    }
}

/// Reads all benchmark results from the given criterion output directory
/// and renders a markdown table.
///
/// `allowlist` filters benchmarks by `full_id`.
///
/// If the iterator is empty, no filtering is applied.
pub fn render(
    criterion_dir: impl AsRef<Path>,
    allowlist: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<String> {
    Renderer::new(criterion_dir).benchmarks(allowlist).render()
}

/// Like [`render`], but accepts additional [`RenderOptions`] to control output.
pub fn render_with_options(
    criterion_dir: impl AsRef<Path>,
    allowlist: impl IntoIterator<Item = impl AsRef<str>>,
    options: &RenderOptions,
) -> Result<String> {
    let renderer = Renderer::new(criterion_dir)
        .benchmarks(allowlist)
        .baseline(&options.baseline)
        .title(&options.title)
        .collapsible(options.collapsible);
    renderer.render()
}
