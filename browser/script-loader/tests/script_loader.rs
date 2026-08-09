use std::collections::BTreeMap;

use tsvm_interop::HostEnvironment;
use tsvm_interpreter::Value;
use tsvm_script_loader::{
    execute_typescript_scripts, execute_typescript_scripts_with_policy, PageScriptSession,
    ScriptKind, ScriptPolicy,
};

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

#[test]
fn script_policy_can_block_typescript_execution() {
    let err = execute_typescript_scripts_with_policy(
        "/index.html",
        r#"<script type="text/typescript">console.log(42);</script>"#,
        &BTreeMap::new(),
        &HostEnvironment::new(),
        ScriptPolicy {
            allow_typescript: false,
        },
    )
    .expect_err("policy should block TypeScript");

    assert!(err.message.contains("blocked"));
}

#[test]
fn session_policy_blocks_before_cache_lookup() {
    let mut session = PageScriptSession::new(1).expect("capacity should be valid");
    let error = session
        .execute_inline_typescript(
            "console.log(42);",
            &HostEnvironment::new(),
            ScriptPolicy {
                allow_typescript: false,
            },
        )
        .expect_err("policy should reject before preparation");

    assert!(error.message.contains("blocked"));
    assert_eq!(session.cache_stats().hits, 0);
    assert_eq!(session.cache_stats().misses, 0);
    assert_eq!(session.cache_stats().entries, 0);
}

#[test]
fn session_cache_reuse_starts_each_execution_with_fresh_runtime_state() {
    let mut session = PageScriptSession::new(1).expect("capacity should be valid");
    let source = "const state = { count: 1 }; state.count += 1; console.log(state.count);";

    let first = session
        .execute_inline_typescript(source, &HostEnvironment::new(), ScriptPolicy::default())
        .expect("first execution should succeed");
    let second = session
        .execute_inline_typescript(source, &HostEnvironment::new(), ScriptPolicy::default())
        .expect("cached execution should succeed");

    assert_eq!(first.console, vec![Value::Number(2.0)]);
    assert_eq!(second.console, vec![Value::Number(2.0)]);
    assert_eq!(session.cache_stats().hits, 1);
    assert_eq!(session.cache_stats().misses, 1);
}

#[test]
fn session_cache_reuse_uses_the_current_host_environment() {
    fn host_value_41(
        _args: &[tsvm_interop::InteropValue],
    ) -> Result<tsvm_interop::InteropValue, tsvm_interop::InteropError> {
        Ok(tsvm_interop::InteropValue::Number(41.0))
    }

    fn host_value_42(
        _args: &[tsvm_interop::InteropValue],
    ) -> Result<tsvm_interop::InteropValue, tsvm_interop::InteropError> {
        Ok(tsvm_interop::InteropValue::Number(42.0))
    }

    let mut session = PageScriptSession::new(1).expect("capacity should be valid");
    let source = "function pageValue(): number { return 0; } console.log(pageValue());";
    let first_host = HostEnvironment::new().with_function("pageValue", host_value_41);
    let second_host = HostEnvironment::new().with_function("pageValue", host_value_42);

    let first = session
        .execute_inline_typescript(source, &first_host, ScriptPolicy::default())
        .expect("first execution should succeed");
    let second = session
        .execute_inline_typescript(source, &second_host, ScriptPolicy::default())
        .expect("cached execution should succeed");

    assert_eq!(first.console, vec![Value::Number(41.0)]);
    assert_eq!(second.console, vec![Value::Number(42.0)]);
    assert_eq!(session.cache_stats().hits, 1);
    assert_eq!(session.cache_stats().misses, 1);
}

#[test]
fn session_treats_changed_inline_source_as_a_cache_miss() {
    let mut session = PageScriptSession::new(2).expect("capacity should be valid");

    session
        .execute_inline_typescript(
            "console.log(40 + 2);",
            &HostEnvironment::new(),
            ScriptPolicy::default(),
        )
        .expect("first source should execute");
    let output = session
        .execute_inline_typescript(
            "console.log(40 + 3);",
            &HostEnvironment::new(),
            ScriptPolicy::default(),
        )
        .expect("changed source should execute");

    assert_eq!(output.console, vec![Value::Number(43.0)]);
    assert_eq!(session.cache_stats().hits, 0);
    assert_eq!(session.cache_stats().misses, 2);
    assert_eq!(session.cache_stats().entries, 2);
}

#[test]
fn session_caches_resolved_external_module_source() {
    let mut session = PageScriptSession::new(1).expect("capacity should be valid");
    let html = r#"<script type="text/typescript" src="/app.ts"></script>"#;
    let resources = BTreeMap::from([("/app.ts".into(), "console.log(42);".into())]);

    let first = session
        .execute_typescript_scripts_with_policy(
            "/index.html",
            html,
            &resources,
            &HostEnvironment::new(),
            ScriptPolicy::default(),
        )
        .expect("resolved module should execute");
    let second = session
        .execute_typescript_scripts_with_policy(
            "/index.html",
            html,
            &resources,
            &HostEnvironment::new(),
            ScriptPolicy::default(),
        )
        .expect("cached resolved module should execute");

    assert_eq!(first.console, vec![Value::Number(42.0)]);
    assert_eq!(second.console, vec![Value::Number(42.0)]);
    assert!(!second.generated_javascript);
    assert_eq!(session.cache_stats().hits, 1);
    assert_eq!(session.cache_stats().misses, 1);
}
