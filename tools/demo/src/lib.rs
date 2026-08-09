#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use tsvm_benchmarks::{csv_header, run_default_benchmarks};
use tsvm_interpreter::{execute_source, Value};
use tsvm_script_loader::execute_typescript_scripts;
use tsvm_web_bindings::{BrowserBindings, Document, FetchService};

const INITIAL_DEMO: &str = r#"
interface Account {
  id: number;
  balance: number;
}

function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
"#;

const DOM_FETCH_DEMO: &str = r##"
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
"##;

pub fn render_demo() -> String {
    let mut out = String::new();
    out.push_str("TSVM Demo\n");
    out.push_str("=========\n\n");
    out.push_str("Pipeline proof:\n");
    out.push_str(
        "TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM\n",
    );
    out.push_str("Generated JavaScript: no\n\n");

    render_initial_demo(&mut out);
    render_script_loader_demo(&mut out);
    render_dom_fetch_demo(&mut out);
    render_cross_origin_demo(&mut out);
    render_benchmarks(&mut out);

    out
}

fn render_initial_demo(out: &mut String) {
    out.push_str("Initial standalone TypeScript:\n");
    out.push_str(trimmed_source(INITIAL_DEMO));
    out.push('\n');

    let execution = execute_source(INITIAL_DEMO).expect("initial demo should execute");
    out.push_str(&format!(
        "Initial demo console: {}\n\n",
        values(&execution.console)
    ));
}

fn render_script_loader_demo(out: &mut String) {
    let html = r#"<script src="/legacy.js"></script>
<script type="text/typescript" src="/app.ts"></script>"#;
    let resources = BTreeMap::from([(
        "/app.ts".into(),
        "const answer: number = 42;\nconsole.log(answer);".into(),
    )]);
    let execution =
        execute_typescript_scripts("/index.html", html, &resources).expect("script loader demo");

    out.push_str("Browser script-loader model:\n");
    out.push_str("<script type=\"text/typescript\" src=\"/app.ts\"></script>\n");
    out.push_str(&format!(
        "Script loader console: {}\n",
        values(&execution.console)
    ));
    out.push_str(&format!(
        "Generated JavaScript: {}\n\n",
        if execution.generated_javascript {
            "yes"
        } else {
            "no"
        }
    ));
}

fn render_dom_fetch_demo(out: &mut String) {
    let bindings = BrowserBindings::new(
        Document::from_text_nodes([("#app", "")]),
        FetchService::new(
            "https://example.test",
            BTreeMap::from([("/message.txt".into(), "hello from TSVM".into())]),
        ),
    );

    let execution =
        tsvm_interpreter::execute_source_with_host(DOM_FETCH_DEMO, &bindings.host_environment())
            .expect("DOM/fetch demo should execute");
    let text = bindings.document().text("#app").unwrap_or_default();

    out.push_str("DOM/fetch host bindings:\n");
    out.push_str(&format!("DOM #app after fetch: {text}\n"));
    out.push_str(&format!(
        "DOM/fetch console: {}\n\n",
        values(&execution.console)
    ));
}

fn render_cross_origin_demo(out: &mut String) {
    let bindings = BrowserBindings::new(
        Document::from_text_nodes([("#app", "")]),
        FetchService::new("https://example.test", BTreeMap::new()),
    );
    let source = r#"
function fetchText(url: string): string {
  return "";
}

console.log(fetchText("https://evil.test/message.txt"));
"#;
    let blocked =
        tsvm_interpreter::execute_source_with_host(source, &bindings.host_environment()).is_err();
    out.push_str(&format!(
        "Cross-origin fetch: {}\n\n",
        if blocked { "blocked" } else { "allowed" }
    ));
}

fn render_benchmarks(out: &mut String) {
    out.push_str("Benchmark snapshot:\n");
    out.push_str(csv_header());
    out.push('\n');
    for result in run_default_benchmarks(3).expect("benchmark snapshot should execute") {
        out.push_str(&result.csv_row());
        out.push('\n');
    }
}

fn trimmed_source(source: &str) -> &str {
    source.trim()
}

fn values(values: &[Value]) -> String {
    values
        .iter()
        .map(render_value)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Boolean(value) => value.to_string(),
        Value::Null => "null".into(),
        Value::Undefined => "undefined".into(),
        Value::Object(_) => "[object Object]".into(),
        Value::Array(values) => values
            .iter()
            .map(render_value)
            .collect::<Vec<_>>()
            .join(","),
    }
}
