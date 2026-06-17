use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{BenchEntry, BenchmarkMeta, ChangeEstimates, ChangeInfo, Estimates};

/// Discovers all benchmark entries by walking the criterion directory.
pub(crate) fn discover_benchmarks(criterion_dir: &Path) -> Result<Vec<BenchEntry>> {
    let mut entries = Vec::new();
    walk_for_benchmarks(criterion_dir, &mut entries)?;
    Ok(entries)
}

/// Recursively walks directories looking for `new/benchmark.json` files.
fn walk_for_benchmarks(dir: &Path, entries: &mut Vec<BenchEntry>) -> Result<()> {
    let new_dir = dir.join("new");
    if new_dir.join("benchmark.json").exists() {
        if let Some(entry) = read_benchmark_entry(&new_dir)? {
            entries.push(entry);
        }
        return Ok(());
    }

    let read_dir = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory {}", dir.display()))?;

    for entry in read_dir {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Skip non-benchmark directories
            if name_str == "reports" || name_str.starts_with('.') {
                continue;
            }
            walk_for_benchmarks(&entry.path(), entries)?;
        }
    }

    Ok(())
}

/// Reads a single benchmark entry from a `new/` directory.
fn read_benchmark_entry(new_dir: &Path) -> Result<Option<BenchEntry>> {
    let meta_path = new_dir.join("benchmark.json");
    let estimates_path = new_dir.join("estimates.json");

    if !estimates_path.exists() {
        return Ok(None);
    }

    let meta: BenchmarkMeta = serde_json::from_str(
        &std::fs::read_to_string(&meta_path)
            .with_context(|| format!("Failed to read {}", meta_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", meta_path.display()))?;

    let estimates: Estimates = serde_json::from_str(
        &std::fs::read_to_string(&estimates_path)
            .with_context(|| format!("Failed to read {}", estimates_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", estimates_path.display()))?;

    // Prefer slope (linear regression) over mean, matching criterion's "typical" behavior
    let estimate_ns = estimates
        .slope
        .as_ref()
        .unwrap_or(&estimates.mean)
        .point_estimate;

    // Read change/estimates.json (sibling to new/) if it exists
    let change_path = new_dir
        .parent()
        .map(|p| p.join("change").join("estimates.json"));
    let change = change_path.filter(|p| p.exists()).and_then(|p| {
        let data = std::fs::read_to_string(&p).ok()?;
        let ce: ChangeEstimates = serde_json::from_str(&data).ok()?;
        Some(ChangeInfo {
            point_estimate: ce.mean.point_estimate,
        })
    });

    Ok(Some(BenchEntry {
        full_id: meta.full_id,
        group_id: meta.group_id,
        function_id: meta.function_id,
        value_str: meta.value_str,
        throughput: meta.throughput,
        estimate_ns,
        change,
    }))
}
