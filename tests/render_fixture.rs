use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

#[test]
fn renders_markdown_from_fixture_criterion_results() {
    let dir = write_fixture(FixtureSpec {
        test_name: "baseline-fixture",
        group_id: "example_group",
        function_id: "sum/1000",
        value_str: None,
        full_id: "example_group/sum/1000",
        slope_ns: 12_000.0,
        mean_ns: 12_345.0,
        change: Some(-0.1),
    });
    let output = criterion_markdown::render(&dir, std::iter::empty::<&str>())
        .expect("render should succeed");

    assert!(output.contains("# Benchmarks"));
    assert!(output.contains("## Benchmark Results"));
    assert!(output.contains("### example_group"));
    assert!(output.contains("`sum`"));
    assert!(output.contains("**`1000`**"));
    assert!(output.contains("`12.00 µs`"));
    assert!(output.contains("✅ **1.11x faster**"));
}

#[test]
fn renders_placeholder_when_change_file_missing() {
    let dir = write_fixture(FixtureSpec {
        test_name: "missing-change",
        group_id: "missing_change_group",
        function_id: "sum/2000",
        value_str: None,
        full_id: "missing_change_group/sum/2000",
        slope_ns: 2_000.0,
        mean_ns: 2_100.0,
        change: None,
    });

    let output = criterion_markdown::render(&dir, std::iter::empty::<&str>())
        .expect("render should succeed");

    assert!(output.contains("### missing_change_group"));
    assert!(output.contains("(---)"));
}

#[test]
fn applies_allowlist_filter() {
    let dir = write_fixture(FixtureSpec {
        test_name: "allowlist",
        group_id: "example_group",
        function_id: "sum/1000",
        value_str: None,
        full_id: "example_group/sum/1000",
        slope_ns: 12_000.0,
        mean_ns: 12_345.0,
        change: Some(-0.1),
    });

    let selected = vec!["example_group/sum/1000".to_string()];
    let output =
        criterion_markdown::render(&dir, &selected).expect("allowlisted render should succeed");
    assert!(output.contains("### example_group"));

    let excluded = vec!["not-a-real-benchmark".to_string()];
    let err = criterion_markdown::render(&dir, &excluded)
        .expect_err("render should fail when no entries match allowlist");
    assert!(err.to_string().contains("No benchmark results found"));
}

#[test]
fn renders_special_characters_in_labels() {
    let dir = write_fixture(FixtureSpec {
        test_name: "special-chars",
        group_id: "special_group",
        function_id: "sum-special/10|20",
        value_str: Some("v|1"),
        full_id: "special_group/sum-special/10|20",
        slope_ns: 3_000.0,
        mean_ns: 3_100.0,
        change: Some(-0.05),
    });

    let output = criterion_markdown::render(&dir, std::iter::empty::<&str>())
        .expect("render should succeed");

    assert!(output.contains("### special_group"));
    assert!(output.contains("sum-special/10|20"));
    assert!(output.contains("**`v|1`**"));
}

#[test]
fn handles_non_positive_change_ratio_as_not_available() {
    let dir = write_fixture(FixtureSpec {
        test_name: "invalid-ratio",
        group_id: "invalid_ratio_group",
        function_id: "sum/10",
        value_str: None,
        full_id: "invalid_ratio_group/sum/10",
        slope_ns: 100.0,
        mean_ns: 120.0,
        change: Some(-1.0),
    });

    let output = criterion_markdown::render(&dir, std::iter::empty::<&str>())
        .expect("render should succeed");

    assert!(output.contains("⚠ n/a"));
}

struct FixtureSpec<'a> {
    test_name: &'a str,
    group_id: &'a str,
    function_id: &'a str,
    value_str: Option<&'a str>,
    full_id: &'a str,
    slope_ns: f64,
    mean_ns: f64,
    change: Option<f64>,
}

fn write_fixture(spec: FixtureSpec<'_>) -> PathBuf {
    let base = unique_temp_fixture_dir(spec.test_name);
    let new_dir = base.join(spec.group_id).join("bench").join("new");
    fs::create_dir_all(&new_dir).expect("failed to create fixture directories");

    let benchmark = json!({
        "group_id": spec.group_id,
        "function_id": spec.function_id,
        "value_str": spec.value_str,
        "throughput": null,
        "full_id": spec.full_id,
    });
    fs::write(
        new_dir.join("benchmark.json"),
        serde_json::to_string_pretty(&benchmark).expect("serialize benchmark fixture"),
    )
    .expect("write benchmark fixture");

    let estimates = json!({
        "mean": { "point_estimate": spec.mean_ns },
        "slope": { "point_estimate": spec.slope_ns },
    });
    fs::write(
        new_dir.join("estimates.json"),
        serde_json::to_string_pretty(&estimates).expect("serialize estimates fixture"),
    )
    .expect("write estimates fixture");

    if let Some(change_point_estimate) = spec.change {
        let change_dir = base.join(spec.group_id).join("bench").join("change");
        fs::create_dir_all(&change_dir).expect("failed to create change fixture directory");
        let change_estimates = json!({
            "mean": { "point_estimate": change_point_estimate }
        });
        fs::write(
            change_dir.join("estimates.json"),
            serde_json::to_string_pretty(&change_estimates).expect("serialize change fixture"),
        )
        .expect("write change fixture");
    }

    base
}

fn unique_temp_fixture_dir(test_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after UNIX_EPOCH")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("criterion-markdown-{test_name}-{nanos}"));
    fs::create_dir_all(&dir).expect("failed to create temp fixture root");
    dir
}
