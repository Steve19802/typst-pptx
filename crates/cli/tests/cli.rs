//! Command-line contract (plan §8): exit codes and error messages.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn compile(deck: &str) -> std::process::Output {
    compile_with(deck, &[])
}

fn compile_with(deck: &str, extra: &[&str]) -> std::process::Output {
    let root = workspace();
    let out = std::env::temp_dir().join(format!("typst-pptx-cli-{}.pptx", std::process::id()));
    Command::new(env!("CARGO_BIN_EXE_typst-pptx"))
        .current_dir(&root)
        .args(["compile", "--root", ".", "--font-path", "fixtures/fonts"])
        .args([
            "--ignore-system-fonts",
            "--package-path",
            "fixtures/packages",
        ])
        .args(["--package-cache-path", "target/no-package-cache"])
        .args(["--diagnostic-format", "short"])
        .args(extra)
        .arg(deck)
        .arg(out)
        .output()
        .unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = workspace().join("target/cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn touying_0_7_is_rejected_with_an_upgrade_hint() {
    let output = compile("fixtures/decks/errors/touying-0.7.typ");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("error: typst-pptx needs Touying 0.8 or newer (found 0.7.4)"),
        "{stderr}"
    );
    assert!(stderr.contains("hint: update the import to @preview/touying:0.8.0"));
}

#[test]
fn compile_errors_exit_with_1() {
    std::fs::write(scratch("broken.typ"), "#let x = (\n").unwrap();
    let output = compile("target/cli-test/broken.typ");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("error:"), "{stderr}");
}

#[test]
fn touying_0_8_exports() {
    let output = compile("fixtures/decks/touying-basic.typ");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn report_and_warnings_point_at_the_source() {
    let report = scratch("fallbacks-report.json");
    let output = compile_with(
        "fixtures/decks/fallbacks.typ",
        &["--report", report.to_str().unwrap()],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    // Same format and position as Typst's own short diagnostics.
    assert!(
        stderr.contains(
            "fixtures/decks/fallbacks.typ:9:31: warning: exported as a picture: rotated, scaled or skewed text"
        ),
        "{stderr}"
    );
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report).unwrap()).unwrap();
    assert_eq!(report["slides"][0]["page"], 1);
    assert_eq!(report["slides"][0]["items"]["native"], 2);
    let first = &report["baked"][0];
    assert_eq!(first["reason"], "rotated, scaled or skewed text");
    assert_eq!(first["file"], "fixtures/decks/fallbacks.typ");
    assert_eq!(
        (first["line"].as_u64(), first["column"].as_u64()),
        (Some(9), Some(31))
    );
}

#[test]
fn flow_text_wraps_in_powerpoint() {
    let ir = scratch("features-flow.json");
    let output = compile_with(
        "fixtures/decks/features.typ",
        &["--text", "flow", "--emit-ir", ir.to_str().unwrap()],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let deck: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ir).unwrap()).unwrap();
    let paragraph = deck["slides"][1]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["wrap"] == true)
        .expect("a wrapping text box");
    let lines = paragraph["paragraphs"][0]["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 1);
    let text: String = lines[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["text"].as_str().unwrap())
        .collect();
    assert!(
        text.starts_with("A left-aligned paragraph that wraps over several lines"),
        "{text}"
    );
}
