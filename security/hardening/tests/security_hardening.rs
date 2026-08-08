use std::collections::BTreeMap;

use tsvm_bytecode::{compile_source, decode_module, Opcode};
use tsvm_interop::HostEnvironment;
use tsvm_interpreter::{execute_module, ExecuteError};
use tsvm_modules::{bundle_module_graph, ModuleDiagnosticCode};
use tsvm_script_loader::{execute_typescript_scripts_with_policy, ScriptPolicy};
use tsvm_web_bindings::{BrowserBindings, Document, FetchService};

#[test]
fn interpreter_refuses_bytecode_that_fails_verification() {
    let mut module = compile_source("const answer: number = 42;")
        .module
        .expect("source should compile");
    module.functions[0].blocks[0].instructions[0].opcode = Opcode::Jump;
    module.functions[0].blocks[0].instructions[0].operands = vec![u32::MAX];

    let err = execute_module(&module).expect_err("invalid bytecode must not execute");

    assert!(matches!(err, ExecuteError::Verify(_)));
}

#[test]
fn script_policy_blocks_typescript_before_compilation() {
    let err = execute_typescript_scripts_with_policy(
        "/index.html",
        r#"<script type="text/typescript">console.log(42);</script>"#,
        &BTreeMap::new(),
        &HostEnvironment::new(),
        ScriptPolicy {
            allow_typescript: false,
        },
    )
    .expect_err("script policy should block TypeScript");

    assert!(err.source.is_none());
}

#[test]
fn module_policy_rejects_remote_specifiers() {
    let err = bundle_module_graph(
        "/app.ts",
        &BTreeMap::from([(
            "/app.ts".into(),
            r#"import { value } from "https://evil.test/mod.ts";"#.into(),
        )]),
    )
    .expect_err("remote specifier should fail");

    assert_eq!(err[0].code, ModuleDiagnosticCode::UnsupportedSpecifier);
}

#[test]
fn fetch_policy_rejects_cross_origin_before_data_enters_tsvm() {
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

    let err = tsvm_interpreter::execute_source_with_host(source, &bindings.host_environment())
        .expect_err("cross-origin fetch should fail");

    assert!(matches!(err, ExecuteError::Interop(_)));
}

#[test]
fn malformed_bytecode_crash_corpus_decodes_as_errors() {
    let corpus = [
        &[][..],
        b"TSVM",
        b"NOPE\x01\x00\x00\x00",
        b"TSVM\x01\x00\xff\xff\xff\xff",
    ];

    for bytes in corpus {
        assert!(decode_module(bytes).is_err());
    }
}
