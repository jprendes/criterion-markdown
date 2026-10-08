use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::{BenchEntry, ChangeInfo};
use crate::{Emojis, Thresholds};

/// Formats all benchmark entries into a markdown string.
///
/// When `skip_title` is true, the top-level report heading is omitted
/// because the output is wrapped in a `<details><summary>` tag.
pub(crate) fn format_table(
    entries: &[BenchEntry],
    title: &str,
    skip_title: bool,
    thresholds: Thresholds,
    emojis: &Emojis,
    summary: &SummaryInfo,
) -> String {
    // Group entries by group_id, preserving discovery order
    let mut groups: BTreeMap<&str, Vec<&BenchEntry>> = BTreeMap::new();
    for entry in entries {
        groups.entry(&entry.group_id).or_default().push(entry);
    }
    let show_comparisons = entries.iter().any(|entry| entry.change.is_some());

    let mut out = String::new();
    if !skip_title {
        writeln!(out, "# {title}\n").unwrap();
    }

    write_summary(&mut out, summary);

    writeln!(out, "## Benchmark Results\n").unwrap();

    for (group_id, group_entries) in &groups {
        writeln!(out, "### {group_id}\n").unwrap();
        write_group_table(
            &mut out,
            group_entries,
            thresholds,
            emojis,
            show_comparisons,
        );
        writeln!(out).unwrap();
    }

    out
}

/// Writes a markdown table for a single benchmark group.
fn write_group_table(
    out: &mut String,
    entries: &[&BenchEntry],
    thresholds: Thresholds,
    emojis: &Emojis,
    show_comparisons: bool,
) {
    // Collect unique functions (columns) and values (rows), preserving order
    let mut functions: Vec<&str> = Vec::new();
    let mut values: Vec<Option<&str>> = Vec::new();

    for entry in entries {
        let col = entry.column();
        if !functions.contains(&col) {
            functions.push(col);
        }
        let row = entry.row();
        if !values.contains(&row) {
            values.push(row);
        }
    }

    // Build a lookup: (column, row) -> &BenchEntry
    let mut lookup: BTreeMap<(&str, Option<&str>), &BenchEntry> = BTreeMap::new();
    for entry in entries {
        lookup.insert((entry.column(), entry.row()), entry);
    }

    // Header row
    write!(out, "|").unwrap();
    // Row label column (empty header)
    write!(out, "            ").unwrap();
    for func in &functions {
        write!(out, " | `{func}`").unwrap();
    }
    writeln!(out, " |").unwrap();

    // Alignment row
    write!(out, "|:-----------|").unwrap();
    for _ in &functions {
        write!(out, ":------------------------ |").unwrap();
    }
    writeln!(out).unwrap();

    // Data rows
    for val in &values {
        let row_label = match val {
            Some(v) => format!("**`{v}`**"),
            None => String::new(),
        };
        write!(out, "| {row_label:10} ").unwrap();

        for func in &functions {
            if let Some(&entry) = lookup.get(&(*func, *val)) {
                let time_str = format_time(entry.estimate_ns);
                if show_comparisons {
                    let change_str = format_change(&entry.change, thresholds, emojis);
                    write!(out, " | `{time_str}` ({change_str}) ").unwrap();
                } else {
                    write!(out, " | `{time_str}` ").unwrap();
                }
            } else {
                write!(out, " |                          ").unwrap();
            }
        }
        writeln!(out, " |").unwrap();
    }
}

/// Formats change vs baseline with tiered emojis (matching criterion-table style).
///
/// Uses `compare = baseline / candidate` and the configured thresholds to
/// determine the indicator tier.
fn format_change(change: &Option<ChangeInfo>, thresholds: Thresholds, emojis: &Emojis) -> String {
    let Some(change) = change else {
        return "---".to_string();
    };

    let classification = classify_change(change, thresholds);
    let ChangeClassification::Valid { ratio, kind } = classification else {
        return "⚠ n/a".to_string();
    };

    let speedup_str = if ratio < 1.0 {
        format!("{:.2}x faster", 1.0 / ratio)
    } else if ratio > 1.0 {
        format!("{:.2}x slower", ratio)
    } else {
        format!("{ratio:.2}x")
    };

    match kind {
        ChangeKind::StrongImprovement => {
            format!("{} **{speedup_str}**", emojis.strong_improvement)
        }
        ChangeKind::Improvement => format!("{} **{speedup_str}**", emojis.improvement),
        ChangeKind::Neutral => format!("{} **{speedup_str}**", emojis.stable),
        ChangeKind::Regression => format!("{} *{speedup_str}*", emojis.regression),
    }
}

fn format_compact_change(change: &ChangeInfo, thresholds: Thresholds, emojis: &Emojis) -> String {
    let ChangeClassification::Valid { ratio, kind } = classify_change(change, thresholds) else {
        return "⚠ n/a".to_string();
    };
    let icon = match kind {
        ChangeKind::StrongImprovement => &emojis.strong_improvement,
        ChangeKind::Improvement => &emojis.improvement,
        ChangeKind::Neutral => &emojis.stable,
        ChangeKind::Regression => &emojis.regression,
    };
    let magnitude = if ratio < 1.0 { 1.0 / ratio } else { ratio };
    format!("{icon} {magnitude:.2}x")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChangeKind {
    StrongImprovement,
    Improvement,
    Neutral,
    Regression,
}

enum ChangeClassification {
    Valid { ratio: f64, kind: ChangeKind },
    Invalid,
}

fn classify_change(change: &ChangeInfo, thresholds: Thresholds) -> ChangeClassification {
    let ratio = 1.0 + change.point_estimate;
    if !ratio.is_finite() || ratio <= 0.0 {
        return ChangeClassification::Invalid;
    }

    // A verdict needs the whole interval past the threshold, so a single noisy
    // run cannot announce a change on its own. Where the interval is unusable,
    // the point estimate stands in for both ends.
    let (lower, upper) = match (change.lower_ratio, change.upper_ratio) {
        (lower, upper) if lower.is_finite() && upper.is_finite() && lower > 0.0 => (lower, upper),
        _ => (ratio, ratio),
    };
    let best = 1.0 / lower;
    let worst = 1.0 / upper;

    let kind = if worst >= thresholds.strong_improvement_ratio {
        ChangeKind::StrongImprovement
    } else if worst >= thresholds.improvement_ratio {
        ChangeKind::Improvement
    } else if best > thresholds.regression_ratio {
        ChangeKind::Neutral
    } else {
        ChangeKind::Regression
    };
    ChangeClassification::Valid { ratio, kind }
}

/// Formats a time in nanoseconds to a human-readable string with appropriate units.
fn format_time(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{:.2} ns", ns)
    } else if ns < 1_000_000.0 {
        format!("{:.2} µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.2} ms", ns / 1_000_000.0)
    } else {
        format!("{:.2} s", ns / 1_000_000_000.0)
    }
}

/// A benchmark change shown in the summary.
pub(crate) struct SummaryEntry {
    pub(crate) id: String,
    pub(crate) change: String,
    headline_change: String,
}

/// Summary information for the top improvements and regressions.
pub(crate) struct SummaryInfo {
    pub(crate) gains: Vec<SummaryEntry>,
    pub(crate) regressions: Vec<SummaryEntry>,
    compared_count: usize,
    enabled: bool,
}

impl SummaryInfo {
    pub(crate) fn headline(&self, emojis: &Emojis) -> Option<String> {
        if !self.enabled {
            return None;
        }
        match (self.regressions.first(), self.gains.first()) {
            (Some(regression), Some(gain)) => Some(format!(
                "{} | {}",
                regression.headline_change, gain.headline_change
            )),
            (Some(regression), None) => Some(regression.headline_change.clone()),
            (None, Some(gain)) => Some(gain.headline_change.clone()),
            (None, None) if self.compared_count > 0 => Some(format!("{} stable", emojis.stable)),
            (None, None) => None,
        }
    }
}

/// Computes the top improvements and regressions across all entries.
pub(crate) fn compute_summary(
    entries: &[BenchEntry],
    thresholds: Thresholds,
    emojis: &Emojis,
    limit: usize,
) -> SummaryInfo {
    let compared_count = entries
        .iter()
        .filter(|entry| {
            entry.change.as_ref().is_some_and(|change| {
                matches!(
                    classify_change(change, thresholds),
                    ChangeClassification::Valid { .. }
                )
            })
        })
        .count();
    let mut gains: Vec<&BenchEntry> = entries
        .iter()
        .filter(|entry| {
            entry.change.as_ref().is_some_and(|change| {
                matches!(
                    classify_change(change, thresholds),
                    ChangeClassification::Valid {
                        kind: ChangeKind::Improvement | ChangeKind::StrongImprovement,
                        ..
                    }
                )
            })
        })
        .collect();
    gains.sort_by(|a, b| {
        a.change
            .as_ref()
            .unwrap()
            .point_estimate
            .partial_cmp(&b.change.as_ref().unwrap().point_estimate)
            .unwrap()
    });

    let mut regressions: Vec<&BenchEntry> = entries
        .iter()
        .filter(|entry| {
            entry.change.as_ref().is_some_and(|change| {
                matches!(
                    classify_change(change, thresholds),
                    ChangeClassification::Valid {
                        kind: ChangeKind::Regression,
                        ..
                    }
                )
            })
        })
        .collect();
    regressions.sort_by(|a, b| {
        b.change
            .as_ref()
            .unwrap()
            .point_estimate
            .partial_cmp(&a.change.as_ref().unwrap().point_estimate)
            .unwrap()
    });

    let to_summary_entry = |entry: &BenchEntry| SummaryEntry {
        id: entry.full_id.clone(),
        change: format_change(&entry.change, thresholds, emojis),
        headline_change: format_compact_change(entry.change.as_ref().unwrap(), thresholds, emojis),
    };

    SummaryInfo {
        gains: gains
            .into_iter()
            .take(limit)
            .map(to_summary_entry)
            .collect(),
        regressions: regressions
            .into_iter()
            .take(limit)
            .map(to_summary_entry)
            .collect(),
        compared_count,
        enabled: limit > 0,
    }
}

/// Writes a summary section with the top improvements and regressions.
fn write_summary(out: &mut String, info: &SummaryInfo) {
    if !info.enabled {
        return;
    }

    if info.gains.is_empty() && info.regressions.is_empty() {
        if info.compared_count > 0 {
            writeln!(out, "No benchmark improved or regressed.").unwrap();
            writeln!(out).unwrap();
        }
        return;
    }

    if !info.gains.is_empty() {
        writeln!(out, "## Top improvements\n").unwrap();
        for entry in &info.gains {
            writeln!(out, "- `{}` — {}", entry.id, entry.change).unwrap();
        }
        writeln!(out).unwrap();
    }

    if !info.regressions.is_empty() {
        writeln!(out, "## Top regressions\n").unwrap();
        for entry in &info.regressions {
            writeln!(out, "- `{}` — {}", entry.id, entry.change).unwrap();
        }
        writeln!(out).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::{format_compact_change, SummaryEntry, SummaryInfo};
    use crate::model::ChangeInfo;
    use crate::{Emojis, Thresholds};

    #[test]
    fn compact_change_preserves_modest_improvement_icon() {
        let change = ChangeInfo {
            point_estimate: 1.0 / 1.2 - 1.0,
            lower_ratio: 1.0 / 1.2,
            upper_ratio: 1.0 / 1.2,
        };

        assert_eq!(
            format_compact_change(&change, Thresholds::default(), &Emojis::default()),
            "↗️ 1.20x"
        );
    }

    /// A point estimate past the threshold is not enough on its own. The
    /// interval has to clear it too, otherwise a noisy run reports a change.
    #[test]
    fn compact_change_holds_back_a_verdict_the_interval_does_not_support() {
        let decided = ChangeInfo {
            point_estimate: 2.0 - 1.0,
            lower_ratio: 1.9,
            upper_ratio: 2.1,
        };
        let undecided = ChangeInfo {
            point_estimate: 2.0 - 1.0,
            lower_ratio: 0.8,
            upper_ratio: 3.2,
        };
        let thresholds = Thresholds::default();

        let emojis = Emojis::default();
        assert_eq!(
            format_compact_change(&decided, thresholds, &emojis),
            "❌ 2.00x"
        );
        assert_eq!(
            format_compact_change(&undecided, thresholds, &emojis),
            "➖ 2.00x"
        );
    }

    #[test]
    fn headline_labels_regression_and_improvement() {
        let summary = SummaryInfo {
            gains: vec![SummaryEntry {
                id: "faster".to_string(),
                change: "2.00x faster".to_string(),
                headline_change: "🚀 2.00x".to_string(),
            }],
            regressions: vec![SummaryEntry {
                id: "slower".to_string(),
                change: "1.20x slower".to_string(),
                headline_change: "❌ 1.20x".to_string(),
            }],
            compared_count: 2,
            enabled: true,
        };

        assert_eq!(
            summary.headline(&Emojis::default()).as_deref(),
            Some("❌ 1.20x | 🚀 2.00x")
        );
    }

    #[test]
    fn headline_reports_stable_when_all_comparisons_are_neutral() {
        let summary = SummaryInfo {
            gains: Vec::new(),
            regressions: Vec::new(),
            compared_count: 2,
            enabled: true,
        };

        assert_eq!(
            summary.headline(&Emojis::default().stable("⚪")).as_deref(),
            Some("⚪ stable")
        );
    }
}
