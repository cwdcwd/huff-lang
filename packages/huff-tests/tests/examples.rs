use huff_parser::{parse_source, ParseError};
use huff_tests::{examples_dir, is_unsupported, list_examples};

#[test]
fn all_examples_either_emit_or_fail_cleanly() {
    let dir = examples_dir();
    let files = list_examples(&dir);
    assert!(!files.is_empty(), "expected example files in {:?}", dir);
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&path).unwrap();
        let result = parse_source(&src);
        if is_unsupported(&name) {
            match result {
                Err(ParseError::NotYetSupported(_)) => { /* expected */ }
                Err(other) => panic!("{}: expected NotYetSupported, got {:?}", name, other),
                Ok(_) => panic!("{}: expected NotYetSupported, parsed OK", name),
            }
        } else {
            let f = result.unwrap_or_else(|e| panic!("{} failed to parse: {:?}", name, e));
            let ts = huff_emit_ts::emit(&f);
            assert!(!ts.is_empty(), "{} emitted empty TS", name);
        }
    }
}

#[test]
fn snapshot_supported_examples() {
    let dir = examples_dir();
    let files = list_examples(&dir);
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if is_unsupported(&name) {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let f = parse_source(&src).unwrap();
        let ts = huff_emit_ts::emit(&f);
        // Use insta inline snapshot file naming based on example.
        insta::with_settings!({snapshot_suffix => name.clone()}, {
            insta::assert_snapshot!(ts);
        });
    }
}
