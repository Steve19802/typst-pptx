//! Puts the pinned Typst version into `--version` (tino compares it with
//! `typst --version`), read from the workspace's `typst = "=X.Y.Z"` pin so a
//! Typst update can't leave it stale.

use std::fs;
use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = fs::read_to_string(&manifest).expect("workspace Cargo.toml");
    let version = text
        .lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("typst = \"=")?;
            Some(rest.split('"').next()?.to_string())
        })
        .expect("workspace Cargo.toml pins typst = \"=X.Y.Z\"");
    println!("cargo:rustc-env=TYPST_PIN={version}");
}
