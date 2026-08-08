use std::collections::BTreeMap;

use tsvm_modules::{bundle_module_graph, ModuleDiagnosticCode};

fn sources(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, source)| ((*path).into(), (*source).into()))
        .collect()
}

fn fixture_sources(kind: &str) -> BTreeMap<String, String> {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("modules")
        .join(kind);
    std::fs::read_dir(&fixture_root)
        .expect("module fixtures should exist")
        .map(|entry| {
            let path = entry.expect("fixture entry should be readable").path();
            let name = path
                .file_name()
                .expect("fixture file should have a name")
                .to_string_lossy();
            let source = std::fs::read_to_string(&path).expect("fixture should be readable");
            (format!("/{name}"), source)
        })
        .collect()
}

#[test]
fn bundles_local_imports_dependency_first() {
    let graph = bundle_module_graph(
        "/app.ts",
        &sources(&[
            (
                "/app.ts",
                r#"
import { Account, credit } from "./account.ts";

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
"#,
            ),
            (
                "/account.ts",
                r#"
export interface Account {
  id: number;
  balance: number;
}

export function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}
"#,
            ),
        ]),
    )
    .expect("module graph should bundle");

    assert_eq!(graph.modules[0].specifier, "/account.ts");
    assert_eq!(graph.modules[1].specifier, "/app.ts");
    assert_eq!(graph.modules[0].exports, vec!["Account", "credit"]);
    assert!(graph.bundled_source.contains("interface Account"));
    assert!(graph.bundled_source.contains("function credit"));
    assert!(!graph.bundled_source.contains("import {"));
    assert!(!graph.bundled_source.contains("export function"));
}

#[test]
fn rejects_missing_local_modules() {
    let err = bundle_module_graph(
        "/app.ts",
        &sources(&[("/app.ts", r#"import { credit } from "./account.ts";"#)]),
    )
    .expect_err("missing import should fail");

    assert_eq!(err[0].code, ModuleDiagnosticCode::MissingModule);
}

#[test]
fn rejects_unsupported_non_local_specifiers() {
    let err = bundle_module_graph(
        "/app.ts",
        &sources(&[(
            "/app.ts",
            r#"import { value } from "https://example.test/mod.ts";"#,
        )]),
    )
    .expect_err("remote import should fail");

    assert_eq!(err[0].code, ModuleDiagnosticCode::UnsupportedSpecifier);
}

#[test]
fn rejects_cycles_with_a_deterministic_path() {
    let err = bundle_module_graph(
        "/a.ts",
        &sources(&[
            (
                "/a.ts",
                r#"import { b } from "./b.ts"; export const a = 1;"#,
            ),
            (
                "/b.ts",
                r#"import { a } from "./a.ts"; export const b = 2;"#,
            ),
        ]),
    )
    .expect_err("cycle should fail");

    assert_eq!(err[0].code, ModuleDiagnosticCode::Cycle);
    assert_eq!(err[0].modules, vec!["/a.ts", "/b.ts", "/a.ts"]);
}

#[test]
fn bundles_valid_module_fixture_corpus() {
    let graph = bundle_module_graph("/app.ts", &fixture_sources("valid"))
        .expect("valid module fixture should bundle");

    assert_eq!(
        graph
            .modules
            .iter()
            .map(|module| module.specifier.as_str())
            .collect::<Vec<_>>(),
        vec!["/account.ts", "/app.ts"]
    );
}

#[test]
fn rejects_invalid_cycle_fixture_corpus() {
    let err = bundle_module_graph("/cycle-a.ts", &fixture_sources("invalid"))
        .expect_err("cycle fixture should fail");

    assert_eq!(err[0].code, ModuleDiagnosticCode::Cycle);
}
