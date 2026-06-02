//! Validation harness — runs each example through the transpiler.

use std::path::{Path, PathBuf};

pub fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../skill/references/examples")
}

pub fn list_examples(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().and_then(|s| s.to_str()) == Some("huff") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

pub fn is_unsupported(name: &str) -> bool {
    name.contains("unsupported")
}
