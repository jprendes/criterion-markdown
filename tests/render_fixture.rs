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
    let output = criterion_markdown::Renderer::new(&dir)
        .baseline("base")
        .render()
        .expect("render should succeed");

    assert!(output.contains("# Benchmarks"));
    assert!(output.contains("## Benchmark Results"));
    assert!(output.contains("### example_group"));
    assert!(output.contains("`sum`"));
    assert!(output.contains("**`1000`**"));
    assert!(output.contains("`12.00 µs`"));
    assert!(output.contains("↗️ **1.11x faster**"));
    assert!(!output.contains("## Summary"));
    assert!(output.contains("## Top improvements"));
    assert!(!output.contains("No benchmark improved or regressed."));
    assert!(!output.contains("## Top regressions"));
    assert!(
        output.find("## Top improvements").unwrap() < output.find("## Benchmark Results").unwrap()
    );
}

#[test]
fn renderer_uses_custom_title() {
    let dir = write_fixture(FixtureSpec {
        test_name: "custom-title",
        group_id: "example_group",
        function_id: "sum/1000",
        value_str: None,
        full_id: "example_group/sum/1000",
        slope_ns: 12_000.0,
        mean_ns: 12_345.0,
        change: Some(-0.1),
    });

    let output = criterion_markdown::Renderer::new(&dir)
        .title("Parser benchmarks")
        .render()
        .expect("render should succeed");

    assert!(output.starts_with("# Parser benchmarks\n"));
    assert!(!output.contains("# Benchmarks\n"));
}

#[test]
fn renderer_uses_default_baseline_when_available() {
    let dir = write_fixture(FixtureSpec {
        test_name: "no-default-baseline",
        group_id: "example_group",
        function_id: "sum/1000",
        value_str: None,
        full_id: "example_group/sum/1000",
        slope_ns: 12_000.0,
        mean_ns: 12_345.0,
        change: Some(-0.5),
    });

    let output = criterion_markdown::render(&dir, std::iter::empty::<&str>())
        .expect("render should succeed");

    assert!(output.contains("`12.00 µs`"));
    assert!(output.contains("🚀 **2.00x faster**"));
    assert!(output.contains("## Top improvements"));
}

#[test]
fn omits_comparisons_when_baseline_is_missing() {
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
    assert!(output.contains("`2.00 µs`"));
    assert!(!output.contains("(---)"));
    assert!(!output.contains("improved or regressed"));

    let collapsible = criterion_markdown::Renderer::new(&dir)
        .title("Missing baseline")
        .collapsible(true)
        .render()
        .expect("collapsible render should succeed");
    assert!(collapsible.starts_with("<details>\n<summary>Missing baseline</summary>"));
    assert!(!collapsible.contains("(---)"));
}

#[test]
fn errors_when_explicit_baseline_is_missing() {
    let dir = write_fixture(FixtureSpec {
        test_name: "missing-explicit-baseline",
        group_id: "missing_baseline_group",
        function_id: "sum/2000",
        value_str: None,
        full_id: "missing_baseline_group/sum/2000",
        slope_ns: 2_000.0,
        mean_ns: 2_100.0,
        change: None,
    });

    let error = criterion_markdown::Renderer::new(&dir)
        .baseline("main")
        .render()
        .expect_err("missing explicit baseline should fail");

    assert!(error
        .to_string()
        .contains("Baseline dataset 'main' not found"));
    assert!(error.to_string().contains(&dir.display().to_string()));
}

#[test]
fn renders_placeholder_when_one_benchmark_is_missing_from_baseline() {
    let dir = unique_temp_fixture_dir("partial-baseline");
    write_fixture_to(
        &dir,
        FixtureSpec {
            test_name: "",
            group_id: "partial_group",
            function_id: "missing/1",
            value_str: None,
            full_id: "partial_group/missing/1",
            slope_ns: 100.0,
            mean_ns: 100.0,
            change: None,
        },
    );
    write_fixture_to(
        &dir,
        FixtureSpec {
            test_name: "",
            group_id: "partial_group",
            function_id: "compared/1",
            value_str: None,
            full_id: "partial_group/compared/1",
            slope_ns: 100.0,
            mean_ns: 100.0,
            change: Some(-0.5),
        },
    );

    let output = criterion_markdown::Renderer::new(&dir)
        .baseline("base")
        .render()
        .expect("render should succeed");

    assert!(output.contains("(---)"));
    assert!(output.contains("🚀 **2.00x faster**"));
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
    let full_id = "invalid_ratio_group/sum/10";
    let dir = write_fixture(FixtureSpec {
        test_name: "invalid-ratio",
        group_id: "invalid_ratio_group",
        function_id: "sum/10",
        value_str: None,
        full_id,
        slope_ns: 100.0,
        mean_ns: 120.0,
        change: None,
    });
    write_estimates(&dir.join(full_id).join("base"), 0.0, 0.0);

    let output = criterion_markdown::Renderer::new(&dir)
        .baseline("base")
        .render()
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
        change: Some(-0.5),
    });

    let options = criterion_markdown::RenderOptions {
        title: "Benchmark Results".to_string(),
        collapsible: true,
        baseline: Some("base".to_string()),
    };
    let output =
        criterion_markdown::render_with_options(&dir, std::iter::empty::<&str>(), &options)
            .expect("render should succeed");

    assert!(output.starts_with("<details>\n<summary>Benchmark Results (🚀 2.00x)</summary>"));
    assert!(output.contains("</summary>"));
    assert!(!output.contains("# Benchmarks"));
    assert!(!output.contains("## Summary"));
    assert!(output.contains("## Top improvements"));
    assert!(output.contains("## Benchmark Results"));
    assert!(output.contains("### example_group"));
    assert!(
        output.find("## Top improvements").unwrap() < output.find("## Benchmark Results").unwrap()
    );
    assert!(
        output.find("## Benchmark Results").unwrap() < output.find("### example_group").unwrap()
    );
    assert!(output.trim_end().ends_with("</details>"));
}

#[test]
fn summary_lists_gain_and_regression() {
    let base = unique_temp_fixture_dir("gain-and-regression");

    // Benchmark with an improvement (-0.5 = 2x faster)
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
            change: Some(-0.5),
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

    let output = criterion_markdown::Renderer::new(&base)
        .baseline("base")
        .render()
        .expect("render should succeed");

    assert!(output.contains("## Top improvements"));
    assert!(output.contains("- `perf_group/fast_fn/100`"));
    assert!(output.contains("## Top regressions"));
    assert!(output.contains("- `perf_group/slow_fn/200`"));
    assert!(output.contains("2.00x faster"));
    assert!(output.contains("1.50x slower"));
}

#[test]
fn summary_limits_and_orders_each_category() {
    let base = unique_temp_fixture_dir("top-summary");

    for (full_id, function_id, change) in [
        ("summary_group/gain_1/1", "gain_1/1", -0.6),
        ("summary_group/gain_2/2", "gain_2/2", -0.5),
        ("summary_group/gain_3/3", "gain_3/3", -0.45),
        ("summary_group/regression_1/1", "regression_1/1", 0.5),
        ("summary_group/regression_2/2", "regression_2/2", 0.3),
        ("summary_group/regression_3/3", "regression_3/3", 0.2),
        ("summary_group/unchanged/4", "unchanged/4", 0.0),
    ] {
        write_fixture_to(
            &base,
            FixtureSpec {
                test_name: "",
                group_id: "summary_group",
                function_id,
                value_str: None,
                full_id,
                slope_ns: 100.0,
                mean_ns: 100.0,
                change: Some(change),
            },
        );
    }

    let output = criterion_markdown::Renderer::new(&base)
        .baseline("base")
        .summary_limit(2)
        .render()
        .expect("render with summary limit should succeed");
    let summary = output
        .split_once("## Benchmark Results")
        .expect("summary should precede benchmark results")
        .0;

    let gain_1 = summary.find("summary_group/gain_1/1").unwrap();
    let gain_2 = summary.find("summary_group/gain_2/2").unwrap();
    let regression_1 = summary.find("summary_group/regression_1/1").unwrap();
    let regression_2 = summary.find("summary_group/regression_2/2").unwrap();
    assert!(gain_1 < gain_2);
    assert!(regression_1 < regression_2);
    assert!(!summary.contains("summary_group/gain_3/3"));
    assert!(!summary.contains("summary_group/regression_3/3"));
    assert!(!summary.contains("summary_group/unchanged/4"));
}

#[test]
fn summary_reports_when_no_benchmark_changed() {
    let dir = unique_temp_fixture_dir("unchanged-summary");
    for (full_id, function_id, change) in [
        ("summary_group/neutral_faster/1", "neutral_faster/1", -0.05),
        ("summary_group/neutral_slower/2", "neutral_slower/2", 0.05),
    ] {
        write_fixture_to(
            &dir,
            FixtureSpec {
                test_name: "",
                group_id: "summary_group",
                function_id,
                value_str: None,
                full_id,
                slope_ns: 100.0,
                mean_ns: 100.0,
                change: Some(change),
            },
        );
    }

    let output = criterion_markdown::Renderer::new(&dir)
        .baseline("base")
        .render()
        .expect("unchanged render should succeed");

    assert!(!output.contains("## Summary"));
    assert!(output.contains("# Benchmarks\n\nNo benchmark improved or regressed."));
    assert!(output.contains("➖ **1.05x faster**"));
    assert!(output.contains("➖ **1.05x slower**"));
    assert!(!output.contains("## Top improvements"));
    assert!(!output.contains("## Top regressions"));
}

#[test]
fn disabled_summary_omits_no_change_message() {
    let dir = write_fixture(FixtureSpec {
        test_name: "disabled-summary",
        group_id: "summary_group",
        function_id: "unchanged/1",
        value_str: None,
        full_id: "summary_group/unchanged/1",
        slope_ns: 100.0,
        mean_ns: 100.0,
        change: Some(0.0),
    });

    let output = criterion_markdown::Renderer::new(&dir)
        .baseline("base")
        .summary_limit(0)
        .render()
        .expect("render without summary should succeed");

    assert!(!output.contains("## Summary"));
    assert!(!output.contains("No benchmark improved or regressed."));
}

#[test]
fn computes_change_from_selected_baseline() {
    let full_id = "example_group/sum/1000";
    let dir = write_fixture(FixtureSpec {
        test_name: "selected-baseline",
        group_id: "example_group",
        function_id: "sum/1000",
        value_str: None,
        full_id,
        slope_ns: 120.0,
        mean_ns: 120.0,
        change: Some(0.2),
    });
    write_estimates(&dir.join(full_id).join("previous"), 240.0, 240.0);

    let change_dir = dir.join(full_id).join("change");
    fs::create_dir_all(&change_dir).expect("failed to create change fixture directory");
    fs::write(
        change_dir.join("estimates.json"),
        serde_json::to_string_pretty(&json!({
            "mean": { "point_estimate": 0.5 }
        }))
        .expect("serialize change fixture"),
    )
    .expect("write change fixture");

    let options = criterion_markdown::RenderOptions {
        baseline: Some("previous".to_string()),
        ..Default::default()
    };
    let output =
        criterion_markdown::render_with_options(&dir, std::iter::empty::<&str>(), &options)
            .expect("render should succeed");

    assert!(output.contains("🚀 **2.00x faster**"));
    assert!(!output.contains("1.50x slower"));
}

#[test]
fn renderer_builder_configures_the_full_render() {
    let base = unique_temp_fixture_dir("renderer-builder");
    let included = ["builder_group/fast/1", "builder_group/slow/2"];
    let excluded = "builder_group/excluded/3";

    for (full_id, function_id) in [
        (included[0], "fast/1"),
        (included[1], "slow/2"),
        (excluded, "excluded/3"),
    ] {
        write_fixture_to(
            &base,
            FixtureSpec {
                test_name: "",
                group_id: "builder_group",
                function_id,
                value_str: None,
                full_id,
                slope_ns: 999.0,
                mean_ns: 999.0,
                change: None,
            },
        );
        write_candidate(&base, full_id, "comparison", 100.0, 100.0);
        write_estimates(&base.join(full_id).join("main"), 200.0, 200.0);
    }

    let output = criterion_markdown::Renderer::new(&base)
        .candidate("comparison")
        .baseline("main")
        .benchmark(included[0])
        .benchmarks([included[1]])
        .title("Builder Results")
        .collapsible(true)
        .render()
        .expect("builder render should succeed");

    assert!(output.starts_with("<details>\n<summary>Builder Results (🚀 2.00x)</summary>"));
    assert!(output.contains("`100.00 ns`"));
    assert!(output.contains("🚀 **2.00x faster**"));
    assert!(output.contains("`builder_group/fast/1`"));
    assert!(output.contains("`builder_group/slow/2`"));
    assert!(!output.contains(excluded));
}

#[test]
fn renderer_loads_baseline_from_separate_root() {
    let candidate_root = unique_temp_fixture_dir("candidate-root");
    let baseline_root = unique_temp_fixture_dir("baseline-root");
    let full_id = "external_group/function/1000";
    let benchmark_path = Path::new("external_group/function_1000");

    write_fixture_to(
        &candidate_root,
        FixtureSpec {
            test_name: "",
            group_id: "external_group",
            function_id: "function/1000",
            value_str: None,
            full_id,
            slope_ns: 100.0,
            mean_ns: 100.0,
            change: None,
        },
    );
    fs::rename(
        candidate_root.join(full_id),
        candidate_root.join(benchmark_path),
    )
    .expect("move candidate to sanitized benchmark path");
    write_estimates(
        &baseline_root.join(benchmark_path).join("main"),
        200.0,
        200.0,
    );

    let output = criterion_markdown::Renderer::new(&candidate_root)
        .baseline_root(&baseline_root)
        .baseline("main")
        .render()
        .expect("render with external baseline should succeed");

    assert!(output.contains("🚀 **2.00x faster**"));
}

#[test]
fn renderer_finds_default_baseline_in_separate_root() {
    let candidate_root = unique_temp_fixture_dir("candidate-default-root");
    let baseline_root = unique_temp_fixture_dir("baseline-default-root");
    let full_id = "external_group/function/1000";

    write_fixture_to(
        &candidate_root,
        FixtureSpec {
            test_name: "",
            group_id: "external_group",
            function_id: "function/1000",
            value_str: None,
            full_id,
            slope_ns: 100.0,
            mean_ns: 100.0,
            change: None,
        },
    );
    write_estimates(&baseline_root.join(full_id).join("base"), 200.0, 200.0);

    let output = criterion_markdown::Renderer::new(&candidate_root)
        .baseline_root(&baseline_root)
        .render()
        .expect("render with detected external baseline should succeed");

    assert!(output.contains("🚀 **2.00x faster**"));
}

#[test]
fn renderer_uses_custom_change_thresholds() {
    let base = unique_temp_fixture_dir("change-thresholds");

    for (full_id, function_id, change) in [
        ("threshold_group/faster/1", "faster/1", -0.1),
        ("threshold_group/slower/2", "slower/2", 0.05),
    ] {
        write_fixture_to(
            &base,
            FixtureSpec {
                test_name: "",
                group_id: "threshold_group",
                function_id,
                value_str: None,
                full_id,
                slope_ns: 100.0,
                mean_ns: 100.0,
                change: Some(change),
            },
        );
    }

    let thresholds = criterion_markdown::ChangeThresholds::default()
        .improvement_ratio(1.1)
        .strong_improvement_ratio(1.5)
        .regression_ratio(0.96);
    let output = criterion_markdown::Renderer::new(&base)
        .baseline("base")
        .change_thresholds(thresholds)
        .render()
        .expect("render with custom thresholds should succeed");

    assert!(output.contains("↗️ **1.11x faster**"));
    assert!(output.contains("❌ *1.05x slower*"));
}

#[test]
fn renderer_rejects_invalid_change_thresholds() {
    let dir = write_fixture(FixtureSpec {
        test_name: "invalid-thresholds",
        group_id: "threshold_group",
        function_id: "sum/1",
        value_str: None,
        full_id: "threshold_group/sum/1",
        slope_ns: 100.0,
        mean_ns: 100.0,
        change: None,
    });

    let invalid_improvement =
        criterion_markdown::ChangeThresholds::default().improvement_ratio(0.9);
    let error = criterion_markdown::Renderer::new(&dir)
        .change_thresholds(invalid_improvement)
        .render()
        .expect_err("invalid improvement ratio should fail");
    assert!(error.to_string().contains("improvement ratio"));

    let invalid_regression = criterion_markdown::ChangeThresholds::default().regression_ratio(1.1);
    let error = criterion_markdown::Renderer::new(&dir)
        .change_thresholds(invalid_regression)
        .render()
        .expect_err("invalid regression ratio should fail");
    assert!(error.to_string().contains("regression ratio"));

    let invalid_strong_improvement =
        criterion_markdown::ChangeThresholds::default().strong_improvement_ratio(1.05);
    let error = criterion_markdown::Renderer::new(&dir)
        .change_thresholds(invalid_strong_improvement)
        .render()
        .expect_err("invalid strong improvement ratio should fail");
    assert!(error.to_string().contains("strong improvement ratio"));
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

    write_estimates(&new_dir, spec.mean_ns, spec.slope_ns);

    if let Some(change_point_estimate) = spec.change {
        let baseline_mean_ns = spec.mean_ns / (1.0 + change_point_estimate);
        write_estimates(
            &base.join(spec.full_id).join("base"),
            baseline_mean_ns,
            baseline_mean_ns,
        );
    }
}

fn write_estimates(dir: &Path, mean_ns: f64, slope_ns: f64) {
    fs::create_dir_all(dir).expect("failed to create estimates fixture directory");
    let estimates = json!({
        "mean": { "point_estimate": mean_ns },
        "slope": { "point_estimate": slope_ns },
    });
    fs::write(
        dir.join("estimates.json"),
        serde_json::to_string_pretty(&estimates).expect("serialize estimates fixture"),
    )
    .expect("write estimates fixture");
}

fn write_candidate(base: &Path, full_id: &str, candidate: &str, mean_ns: f64, slope_ns: f64) {
    let benchmark_dir = base.join(full_id);
    let candidate_dir = benchmark_dir.join(candidate);
    fs::create_dir_all(&candidate_dir).expect("failed to create candidate fixture directory");
    fs::copy(
        benchmark_dir.join("new").join("benchmark.json"),
        candidate_dir.join("benchmark.json"),
    )
    .expect("copy candidate benchmark fixture");
    write_estimates(&candidate_dir, mean_ns, slope_ns);
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
