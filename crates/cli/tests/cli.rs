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
        .arg(deck)
        .arg(out)
        .output()
        .unwrap()
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
    let dir = workspace().join("target/cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.typ"), "#let x = (\n").unwrap();
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
