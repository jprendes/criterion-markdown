use std::fs;
use std::path::{Path, PathBuf};
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
    assert!(output.contains("## Summary"));
    assert!(output.contains("**Biggest gain:**"));
    assert!(output.contains("**Least gain:**"));
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

#[test]
fn wraps_in_details_summary_when_option_set() {
    let dir = write_fixture(FixtureSpec {
        test_name: "details-summary",
        group_id: "example_group",
        function_id: "sum/1000",
        value_str: None,
        full_id: "example_group/sum/1000",
        slope_ns: 12_000.0,
        mean_ns: 12_345.0,
        change: Some(-0.1),
    });

    let options = criterion_markdown::RenderOptions {
        collapsible: Some("Benchmark Results".to_string()),
    };
    let output =
        criterion_markdown::render_with_options(&dir, std::iter::empty::<&str>(), &options)
            .expect("render should succeed");

    assert!(output.starts_with("<details>\n<summary>Benchmark Results ("));
    assert!(output.contains("</summary>"));
    assert!(!output.contains("# Benchmarks"));
    assert!(!output.contains("## Benchmark Results"));
    assert!(output.contains("### example_group"));
    assert!(output.trim_end().ends_with("</details>"));
}

#[test]
fn summary_labels_with_gain_and_regression() {
    let base = unique_temp_fixture_dir("gain-and-regression");

    // Benchmark with a gain (-0.3 = 30% faster)
    write_fixture_to(
        &base,
        FixtureSpec {
            test_name: "",
            group_id: "perf_group",
            function_id: "fast_fn/100",
            value_str: None,
            full_id: "perf_group/fast_fn/100",
            slope_ns: 5_000.0,
            mean_ns: 5_100.0,
            change: Some(-0.3),
        },
    );

    // Benchmark with a regression (+0.5 = 50% slower)
    write_fixture_to(
        &base,
        FixtureSpec {
            test_name: "",
            group_id: "perf_group",
            function_id: "slow_fn/200",
            value_str: None,
            full_id: "perf_group/slow_fn/200",
            slope_ns: 10_000.0,
            mean_ns: 10_500.0,
            change: Some(0.5),
        },
    );

    let output = criterion_markdown::render(&base, std::iter::empty::<&str>())
        .expect("render should succeed");

    assert!(output.contains("**Biggest gain:** `perf_group/fast_fn/100`"));
    assert!(output.contains("**Worst regression:** `perf_group/slow_fn/200`"));
    assert!(output.contains("1.43x faster"));
    assert!(output.contains("1.50x slower"));
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
    write_fixture_to(&base, spec);
    base
}

fn write_fixture_to(base: &Path, spec: FixtureSpec<'_>) {
    let new_dir = base.join(spec.full_id).join("new");
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
        let change_dir = base.join(spec.full_id).join("change");
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
