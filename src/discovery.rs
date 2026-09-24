use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{BenchEntry, BenchmarkMeta, ChangeInfo, Estimates};

/// Discovers all benchmark entries by walking the criterion directory.
pub(crate) fn discover_benchmarks(
    criterion_dir: &Path,
    candidate: &str,
    baseline_root: &Path,
    baseline: &str,
) -> Result<Vec<BenchEntry>> {
    let mut entries = Vec::new();
    walk_for_benchmarks(
        criterion_dir,
        criterion_dir,
        candidate,
        baseline_root,
        baseline,
        &mut entries,
    )?;
    Ok(entries)
}

/// Recursively walks directories looking for candidate `benchmark.json` files.
fn walk_for_benchmarks(
    criterion_dir: &Path,
    dir: &Path,
    candidate: &str,
    baseline_root: &Path,
    baseline: &str,
    entries: &mut Vec<BenchEntry>,
) -> Result<()> {
    let candidate_dir = dir.join(candidate);
    if candidate_dir.join("benchmark.json").exists() {
        let benchmark_path = dir.strip_prefix(criterion_dir).with_context(|| {
            format!(
                "Failed to resolve benchmark path {} relative to {}",
                dir.display(),
                criterion_dir.display()
            )
        })?;
        let baseline_dir = baseline_root.join(benchmark_path).join(baseline);
        if let Some(entry) = read_benchmark_entry(&candidate_dir, &baseline_dir)? {
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
            walk_for_benchmarks(
                criterion_dir,
                &entry.path(),
                candidate,
                baseline_root,
                baseline,
                entries,
            )?;
        }
    }

    Ok(())
}

/// Reads a single benchmark entry from a candidate directory.
fn read_benchmark_entry(candidate_dir: &Path, baseline_dir: &Path) -> Result<Option<BenchEntry>> {
    let meta_path = candidate_dir.join("benchmark.json");
    let estimates_path = candidate_dir.join("estimates.json");

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

    let baseline_path = baseline_dir.join("estimates.json");
    let change = baseline_path
        .exists()
        .then_some(baseline_path)
        .and_then(|p| {
            let data = std::fs::read_to_string(&p).ok()?;
            let baseline_estimates: Estimates = serde_json::from_str(&data).ok()?;
            Some(ChangeInfo::from_estimates(&estimates, &baseline_estimates))
        });

    Ok(Some(BenchEntry {
        full_id: meta.full_id,
        group_id: meta.group_id,
        function_id: meta.function_id.unwrap_or_default(),
        value_str: meta.value_str,
        throughput: meta.throughput,
        estimate_ns,
        change,
    }))
}
