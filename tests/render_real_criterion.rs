use std::path::Path;
use std::process::Command;

#[test]
fn renders_markdown_from_real_criterion_output() {
    let status = Command::new("cargo")
        .args(["bench", "--bench", "example_benchmark", "--", "--noplot"])
        .status()
        .expect("failed to execute cargo bench");

    assert!(status.success(), "cargo bench did not finish successfully");

    let output =
        criterion_markdown::render(Path::new("target/criterion"), std::iter::empty::<&str>())
            .expect("render should succeed on real criterion output");

    assert!(output.contains("### example_group"));
    assert!(output.contains("`sum`"));
}
