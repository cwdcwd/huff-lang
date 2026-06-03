use huff_parser::{parse_source, ParseError};

fn read(path: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../skill/references/examples")
        .join(path);
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("read {:?}: {}", p, e))
}

#[test]
fn hello_minimal_parses() {
    let src = read("hello.huff");
    let f = parse_source(&src).expect("hello.huff should parse");
    assert_eq!(f.name, "HelloWorld");
}

#[test]
fn greetings_parses() {
    let src = read("greetings.huff");
    let f = parse_source(&src).expect("greetings.huff should parse");
    assert_eq!(f.name, "Greetings");
}

#[test]
fn counter_parses() {
    let src = read("counter.huff");
    let f = parse_source(&src).expect("counter.huff should parse");
    assert_eq!(f.name, "Counter");
}

#[test]
fn async_parses() {
    let src = read("async.huff");
    let f = parse_source(&src).expect("async.huff should parse");
    assert_eq!(f.name, "AsyncDemo");
    let has_async_op = f.items.iter().any(|it| matches!(it, huff_ast::Item::Op(op) if op.is_async));
    assert!(has_async_op, "expected at least one async op");
}

#[test]
fn match_unsupported_errors_cleanly() {
    let src = read("match_unsupported.huff");
    let err = parse_source(&src).expect_err("must fail");
    match err {
        ParseError::NotYetSupported(msg) => {
            assert!(msg.contains("match"), "expected match error, got {:?}", msg)
        }
        other => panic!("expected NotYetSupported, got {:?}", other),
    }
}

#[test]
fn svc_unsupported_errors_cleanly() {
    let src = read("svc_unsupported.huff");
    let err = parse_source(&src).expect_err("must fail");
    match err {
        ParseError::NotYetSupported(msg) => assert!(msg.contains("svc")),
        other => panic!("expected NotYetSupported, got {:?}", other),
    }
}
