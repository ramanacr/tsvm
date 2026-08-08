use std::collections::BTreeMap;

use tsvm_interpreter::{execute_source_with_host, ExecuteError, Value};
use tsvm_web_bindings::{BrowserBindings, Document, FetchService};

fn script() -> &'static str {
    r##"
function domSetText(selector: string, text: string): undefined {
  return undefined;
}

function domText(selector: string): string {
  return "";
}

function fetchText(url: string): string {
  return "";
}

domSetText("#app", fetchText("/message.txt"));
console.log(domText("#app"));
"##
}

#[test]
fn typescript_fetches_same_origin_text_and_mutates_document() {
    let bindings = BrowserBindings::new(
        Document::from_text_nodes([("#app", "")]),
        FetchService::new(
            "https://example.test",
            BTreeMap::from([("/message.txt".into(), "hello from TSVM".into())]),
        ),
    );

    let output = execute_source_with_host(script(), &bindings.host_environment())
        .expect("DOM/fetch script should execute");

    assert_eq!(
        bindings.document().text("#app").expect("node should exist"),
        "hello from TSVM"
    );
    assert_eq!(
        output.console,
        vec![Value::String("hello from TSVM".into())]
    );
}

#[test]
fn fetch_blocks_cross_origin_urls() {
    let bindings = BrowserBindings::new(
        Document::from_text_nodes([("#app", "")]),
        FetchService::new("https://example.test", BTreeMap::new()),
    );
    let source = r##"
function fetchText(url: string): string {
  return "";
}

console.log(fetchText("https://evil.test/message.txt"));
"##;

    let err = execute_source_with_host(source, &bindings.host_environment())
        .expect_err("cross-origin fetch should fail");

    assert!(matches!(err, ExecuteError::Interop(_)));
}
