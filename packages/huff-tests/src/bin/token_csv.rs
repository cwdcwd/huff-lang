//! Emit `docs/token-counts.csv` measuring BPE token counts of each Huff example
//! against its emitted TypeScript.

use std::path::{Path, PathBuf};

use huff_tests::{examples_dir, is_unsupported, list_examples};
use tiktoken_rs::cl100k_base;

fn main() -> std::io::Result<()> {
    let bpe = cl100k_base().expect("load tokenizer");
    let dir = examples_dir();
    let out_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/token-counts.csv");
    let mut csv = String::new();
    csv.push_str("example,huff_chars,huff_tokens,ts_chars,ts_tokens,ratio\n");
    for path in list_examples(&dir) {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if is_unsupported(&name) {
            continue;
        }
        let huff_src = std::fs::read_to_string(&path)?;
        let file = match huff_parser::parse_source(&huff_src) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("skipping {}: {:?}", name, e);
                continue;
            }
        };
        let ts = huff_emit_ts::emit(&file);
        let huff_tokens = bpe.encode_with_special_tokens(&huff_src).len();
        let ts_tokens = bpe.encode_with_special_tokens(&ts).len();
        let ratio = if huff_tokens == 0 {
            0.0
        } else {
            ts_tokens as f64 / huff_tokens as f64
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{:.2}\n",
            name,
            huff_src.chars().count(),
            huff_tokens,
            ts.chars().count(),
            ts_tokens,
            ratio,
        ));
    }
    std::fs::write(&out_path, &csv)?;
    eprintln!("wrote {}", out_path.display());
    print!("{}", csv);
    Ok(())
}
