//! Reads criterion benchmark results from `target/criterion/` JSON files and
//! renders a markdown table similar to criterion-table.
//!
//! # Example
//!
//! ```rust,no_run
//! fn main() -> anyhow::Result<()> {
//!     let markdown = criterion_markdown::render("target/criterion", std::iter::empty::<&str>())?;
//!     println!("{markdown}");
//!     Ok(())
//! }
//! ```
//!
//! # Collapsible output
//!
//! Use [`RenderOptions::collapsible`] to wrap the output in a
//! `<details><summary>` tag:
//!
//! ```rust,no_run
//! use criterion_markdown::RenderOptions;
//!
//! fn main() -> anyhow::Result<()> {
//!     let options = RenderOptions {
//!         collapsible: Some("Benchmark Results".into()),
//!     };
//!     let markdown = criterion_markdown::render_with_options(
//!         "target/criterion",
//!         std::iter::empty::<&str>(),
//!         &options,
//!     )?;
//!     println!("{markdown}");
//!     Ok(())
//! }
//! ```

use std::path::Path;

use anyhow::Result;

mod discovery;
mod markdown;
mod model;

/// Options for controlling the rendered markdown output.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// If set, wraps the output in a `<details><summary>...</summary>` tag
    /// using this value as the summary text.
    pub collapsible: Option<String>,
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
    render_with_options(criterion_dir, allowlist, &RenderOptions::default())
}

/// Like [`render`], but accepts additional [`RenderOptions`] to control output.
pub fn render_with_options(
    criterion_dir: impl AsRef<Path>,
    allowlist: impl IntoIterator<Item = impl AsRef<str>>,
    options: &RenderOptions,
) -> Result<String> {
    let criterion_dir = criterion_dir.as_ref();
    let mut entries = discovery::discover_benchmarks(criterion_dir)?;
    let names: Vec<String> = allowlist
        .into_iter()
        .map(|n| n.as_ref().to_string())
        .collect();
    if !names.is_empty() {
        entries.retain(|e| names.iter().any(|n| n == &e.full_id));
    }
    if entries.is_empty() {
        anyhow::bail!("No benchmark results found in {}", criterion_dir.display());
    }
    let skip_headers = options.collapsible.is_some();
    let body = markdown::format_table(&entries, skip_headers);
    Ok(match &options.collapsible {
        Some(summary) => {
            let range = markdown::compute_summary(&entries)
                .map(|info| format!(" ({} → {})", info.worst_change, info.best_change))
                .unwrap_or_default();
            format!("<details>\n<summary>{summary}{range}</summary>\n\n{body}\n</details>\n")
        }
        None => body,
    })
}
