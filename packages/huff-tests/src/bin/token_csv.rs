//! Emit `docs/token-counts.csv` measuring token counts of each Huff example
//! against its emitted TypeScript.
//!
//! Two tokenizers are used:
//!
//! 1. `cl100k` — `tiktoken-rs::cl100k_base`, the GPT-4-era BPE. Always run.
//!    Free, offline, deterministic. A reasonable stand-in for "what an LLM
//!    sees" but not actually Claude's tokenizer.
//!
//! 2. `claude` — Anthropic's `messages/count_tokens` endpoint. Only run when
//!    `ANTHROPIC_API_KEY` is set in the environment. The endpoint is free
//!    but rate-limited and returns an *estimate* — the live serving stack
//!    may include small system-added overhead that isn't billed. Model is
//!    overridable via `HUFF_CLAUDE_MODEL` (default: claude-sonnet-4-6).
//!
//! When the Claude path is unavailable, those columns are blank — the CSV
//! still loads cleanly in any spreadsheet/CSV reader.

use std::path::{Path, PathBuf};

use huff_tests::{examples_dir, is_unsupported, list_examples};
use tiktoken_rs::cl100k_base;

const CLAUDE_API: &str = "https://api.anthropic.com/v1/messages/count_tokens";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

fn claude_count(api_key: &str, model: &str, text: &str) -> Result<u64, String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": text}],
    });
    let resp = ureq::post(CLAUDE_API)
        .set("x-api-key", api_key)
        .set("anthropic-version", ANTHROPIC_VERSION)
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|e| format!("HTTP error: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| format!("decode: {e}"))?;
    v.get("input_tokens")
        .and_then(|n| n.as_u64())
        .ok_or_else(|| format!("missing input_tokens in response: {v}"))
}

fn main() -> std::io::Result<()> {
    let bpe = cl100k_base().expect("load tokenizer");
    let dir = examples_dir();
    let out_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/token-counts.csv");

    let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let model = std::env::var("HUFF_CLAUDE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let claude_active = api_key.is_some();
    if claude_active {
        eprintln!("claude tokenizer active (model={})", model);
    } else {
        eprintln!("ANTHROPIC_API_KEY not set — claude_* columns will be blank");
    }

    let mut csv = String::new();
    csv.push_str(
        "example,huff_chars,ts_chars,cl100k_huff_tokens,cl100k_ts_tokens,cl100k_ratio,\
         claude_huff_tokens,claude_ts_tokens,claude_ratio\n",
    );
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

        let cl_huff = bpe.encode_with_special_tokens(&huff_src).len() as u64;
        let cl_ts = bpe.encode_with_special_tokens(&ts).len() as u64;
        let cl_ratio = if cl_huff == 0 { 0.0 } else { cl_ts as f64 / cl_huff as f64 };

        let (claude_huff, claude_ts, claude_ratio_str) = if let Some(key) = &api_key {
            let h = claude_count(key, &model, &huff_src);
            let t = claude_count(key, &model, &ts);
            match (h, t) {
                (Ok(h), Ok(t)) => {
                    let r = if h == 0 { 0.0 } else { t as f64 / h as f64 };
                    (h.to_string(), t.to_string(), format!("{:.2}", r))
                }
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("claude count_tokens failed for {}: {}", name, e);
                    (String::new(), String::new(), String::new())
                }
            }
        } else {
            (String::new(), String::new(), String::new())
        };

        csv.push_str(&format!(
            "{name},{hc},{tc},{ch},{ct},{cr:.2},{kh},{kt},{kr}\n",
            name = name,
            hc = huff_src.chars().count(),
            tc = ts.chars().count(),
            ch = cl_huff,
            ct = cl_ts,
            cr = cl_ratio,
            kh = claude_huff,
            kt = claude_ts,
            kr = claude_ratio_str,
        ));
    }
    std::fs::write(&out_path, &csv)?;
    eprintln!("wrote {}", out_path.display());
    print!("{}", csv);
    Ok(())
}
