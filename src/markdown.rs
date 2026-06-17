use std::collections::BTreeMap;
use std::fmt::Write;

use crate::model::{BenchEntry, ChangeInfo};

/// Formats all benchmark entries into a markdown string.
pub(crate) fn format_table(entries: &[BenchEntry]) -> String {
    // Group entries by group_id, preserving discovery order
    let mut groups: BTreeMap<&str, Vec<&BenchEntry>> = BTreeMap::new();
    for entry in entries {
        groups.entry(&entry.group_id).or_default().push(entry);
    }

    let mut out = String::new();
    writeln!(out, "# Benchmarks\n").unwrap();
    writeln!(out, "## Benchmark Results\n").unwrap();

    for (group_id, group_entries) in &groups {
        writeln!(out, "### {group_id}\n").unwrap();
        write_group_table(&mut out, group_entries);
        writeln!(out).unwrap();
    }

    out
}

/// Writes a markdown table for a single benchmark group.
fn write_group_table(out: &mut String, entries: &[&BenchEntry]) {
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
                let change_str = format_change(&entry.change);
                write!(out, " | `{time_str}` ({change_str}) ").unwrap();
            } else {
                write!(out, " |                          ").unwrap();
            }
        }
        writeln!(out, " |").unwrap();
    }
}

/// Formats change vs baseline with tiered emojis (matching criterion-table style).
///
/// Uses `compare = 1 / ratio` (where ratio = new/old) to determine tier:
/// - `compare >= 1.8` (44%+ faster): 🚀
/// - `compare > 0.9` (within ~10% slower): ✅
/// - `compare <= 0.9` (10%+ slower): ❌
fn format_change(change: &Option<ChangeInfo>) -> String {
    let Some(change) = change else {
        return "---".to_string();
    };

    // ratio = new_time / old_time
    let ratio = 1.0 + change.point_estimate;
    if !ratio.is_finite() || ratio <= 0.0 {
        return "⚠ n/a".to_string();
    }

    // compare = old_time / new_time (criterion-table's convention)
    let compare = 1.0 / ratio;

    let speedup_str = if ratio < 1.0 {
        format!("{:.2}x faster", 1.0 / ratio)
    } else if ratio > 1.0 {
        format!("{:.2}x slower", ratio)
    } else {
        format!("{ratio:.2}x")
    };

    if compare >= 1.8 {
        format!("🚀 **{speedup_str}**")
    } else if compare > 0.9 {
        format!("✅ **{speedup_str}**")
    } else {
        format!("❌ *{speedup_str}*")
    }
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
