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

use std::path::Path;

use anyhow::Result;

mod discovery;
mod markdown;
mod model;

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
    Ok(markdown::format_table(&entries))
}
