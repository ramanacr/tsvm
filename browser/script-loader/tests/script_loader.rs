use std::collections::BTreeMap;

use tsvm_interpreter::Value;
use tsvm_script_loader::{execute_typescript_scripts, ScriptKind};

#[test]
fn executes_text_typescript_src_through_tsvm() {
    let html = r#"
<script src="/legacy.js"></script>
<script type="text/typescript" src="/app.ts"></script>
"#;
    let resources = BTreeMap::from([(
        "/app.ts".into(),
        r#"
const answer: number = 42;
console.log(answer);
"#
        .into(),
    )]);

    let output =
        execute_typescript_scripts("/index.html", html, &resources).expect("page should execute");

    assert_eq!(output.console, vec![Value::Number(42.0)]);
    assert_eq!(output.scripts.len(), 1);
    assert_eq!(output.scripts[0].kind, ScriptKind::External);
    assert_eq!(output.scripts[0].specifier, "/app.ts");
    assert!(!output.generated_javascript);
}

#[test]
fn executes_inline_text_typescript() {
    let output = execute_typescript_scripts(
        "/index.html",
        r#"<script type="text/typescript">console.log(40 + 2);</script>"#,
        &BTreeMap::new(),
    )
    .expect("inline TypeScript should execute");

    assert_eq!(output.console, vec![Value::Number(42.0)]);
    assert_eq!(output.scripts[0].kind, ScriptKind::Inline);
}

#[test]
fn executes_module_imports_from_page_resources() {
    let html = r#"<script type="text/typescript" src="/app.ts"></script>"#;
    let resources = BTreeMap::from([
        (
            "/app.ts".into(),
            r#"
import { answer } from "./answer.ts";
console.log(answer);
"#
            .into(),
        ),
        (
            "/answer.ts".into(),
            "export const answer: number = 42;".into(),
        ),
    ]);

    let output =
        execute_typescript_scripts("/index.html", html, &resources).expect("page should execute");

    assert_eq!(output.console, vec![Value::Number(42.0)]);
}

#[test]
fn reports_missing_typescript_src_resource() {
    let err = execute_typescript_scripts(
        "/index.html",
        r#"<script type="text/typescript" src="/missing.ts"></script>"#,
        &BTreeMap::new(),
    )
    .expect_err("missing script should fail");

    assert!(err.message.contains("/missing.ts"));
}
