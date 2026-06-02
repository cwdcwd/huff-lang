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
fn async_unsupported_errors_cleanly() {
    let src = read("async_unsupported.huff");
    let err = parse_source(&src).expect_err("must fail");
    match err {
        ParseError::NotYetSupported(msg) => assert!(msg.contains("async")),
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
