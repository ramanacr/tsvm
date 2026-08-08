use tsvm_semantic::{analyze_source, DiagnosticCode, SymbolKind};

fn diagnostic_codes(source: &str) -> Vec<DiagnosticCode> {
    analyze_source(source)
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn accepts_initial_demo_and_builds_global_symbols() {
    let source = r#"
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

    let analyzed = analyze_source(source);

    assert!(
        analyzed.diagnostics.is_empty(),
        "{:?}",
        analyzed.diagnostics
    );
    assert!(analyzed
        .symbols
        .types
        .iter()
        .any(|symbol| symbol.name == "Account" && symbol.kind == SymbolKind::Interface));
    assert!(analyzed
        .symbols
        .values
        .iter()
        .any(|symbol| symbol.name == "credit" && symbol.kind == SymbolKind::Function));
    assert!(analyzed
        .symbols
        .values
        .iter()
        .any(|symbol| symbol.name == "account" && symbol.kind == SymbolKind::Variable));
}

#[test]
fn reports_structural_type_mismatch_for_missing_object_property() {
    let codes = diagnostic_codes(
        r#"
interface Account {
  id: number;
  balance: number;
}

const account: Account = { id: 1 };
"#,
    );

    assert!(
        codes.contains(&DiagnosticCode::MissingProperty),
        "{codes:?}"
    );
}

#[test]
fn reports_variable_type_mismatch_and_unknown_identifier() {
    let codes = diagnostic_codes(
        r#"
const amount: number = "50";
const doubled: number = missing + amount;
"#,
    );

    assert!(codes.contains(&DiagnosticCode::TypeMismatch), "{codes:?}");
    assert!(
        codes.contains(&DiagnosticCode::UnknownIdentifier),
        "{codes:?}"
    );
}

#[test]
fn validates_function_call_arity_and_argument_types() {
    let codes = diagnostic_codes(
        r#"
function credit(amount: number, label: string): number {
  return amount;
}

credit("bad");
"#,
    );

    assert!(
        codes.contains(&DiagnosticCode::WrongArgumentCount),
        "{codes:?}"
    );
    assert!(codes.contains(&DiagnosticCode::TypeMismatch), "{codes:?}");
}

#[test]
fn validates_function_return_types_and_local_scopes() {
    let analyzed = analyze_source(
        r#"
function label(value: number): string {
  const suffix: string = "px";
  return value;
}

const suffix: string = "outer";
"#,
    );

    assert!(
        analyzed.diagnostics.iter().any(|diagnostic| diagnostic.code
            == DiagnosticCode::ReturnTypeMismatch
            && diagnostic.span.start.line == 4),
        "{:?}",
        analyzed.diagnostics
    );
    assert!(!analyzed
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::DuplicateSymbol));
}

#[test]
fn semantic_fixture_corpus_matches_expected_validity() {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("semantic");

    for entry in std::fs::read_dir(fixture_root.join("valid")).expect("valid fixtures exist") {
        let path = entry.expect("fixture entry should be readable").path();
        if !path.is_file() {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("fixture should be readable");
        let analyzed = analyze_source(&source);
        assert!(
            analyzed.diagnostics.is_empty(),
            "{} diagnostics: {:?}",
            path.display(),
            analyzed.diagnostics
        );
    }

    for entry in std::fs::read_dir(fixture_root.join("invalid")).expect("invalid fixtures exist") {
        let path = entry.expect("fixture entry should be readable").path();
        if !path.is_file() {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("fixture should be readable");
        let analyzed = analyze_source(&source);
        assert!(
            !analyzed.diagnostics.is_empty(),
            "{} should produce diagnostics",
            path.display()
        );
    }
}
