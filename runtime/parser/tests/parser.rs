use tsvm_ast::{ExpressionKind, StatementKind, TypeKind, VariableKind};
use tsvm_parser::parse_source;

#[test]
fn parses_initial_demo_program_into_spanned_ast() {
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

    let parsed = parse_source(source);

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.program.body.len(), 4);
    assert!(parsed.program.span.is_some());

    let StatementKind::Interface(interface) = &parsed.program.body[0].kind else {
        panic!("expected interface");
    };
    assert_eq!(interface.name, "Account");
    assert_eq!(interface.members.len(), 2);
    assert_eq!(interface.members[0].name, "id");
    assert_eq!(interface.members[0].ty.kind, TypeKind::Number);

    let StatementKind::Function(function) = &parsed.program.body[1].kind else {
        panic!("expected function");
    };
    assert_eq!(function.name, "credit");
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].name, "account");
    assert_eq!(
        function.params[0].ty.as_ref().unwrap().kind,
        TypeKind::Named("Account".into())
    );
    assert_eq!(
        function.return_type.as_ref().unwrap().kind,
        TypeKind::Number
    );
    assert_eq!(function.body.len(), 2);

    let StatementKind::Variable(variable) = &parsed.program.body[2].kind else {
        panic!("expected variable");
    };
    assert_eq!(variable.kind, VariableKind::Const);
    assert_eq!(variable.name, "account");
    assert_eq!(
        variable.ty.as_ref().unwrap().kind,
        TypeKind::Named("Account".into())
    );
    assert!(matches!(
        variable.initializer.as_ref().unwrap().kind,
        ExpressionKind::Object(_)
    ));

    assert!(parsed.program.body[3].span.start.line > parsed.program.body[2].span.start.line);
}

#[test]
fn parses_control_flow_and_expression_precedence() {
    let source = r#"
let x: number = 1 + 2 * 3;
if (x > 3 && x !== null) {
  x += 1;
} else {
  throw "bad";
}
"#;

    let parsed = parse_source(source);

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.program.body.len(), 2);

    let StatementKind::Variable(variable) = &parsed.program.body[0].kind else {
        panic!("expected variable");
    };
    let ExpressionKind::Binary { right, .. } = &variable.initializer.as_ref().unwrap().kind else {
        panic!("expected binary initializer");
    };
    assert!(matches!(right.kind, ExpressionKind::Binary { .. }));

    let StatementKind::If(if_stmt) = &parsed.program.body[1].kind else {
        panic!("expected if statement");
    };
    assert!(matches!(if_stmt.test.kind, ExpressionKind::Binary { .. }));
    assert!(if_stmt.alternate.is_some());
}

#[test]
fn parses_import_export_type_alias_class_and_arrays() {
    let source = r#"
import { value } from "./module.ts";
export type Pair = { left: number; right: number; };
class Box {
  values: number[];
}
const items: number[] = [1, 2, value];
"#;

    let parsed = parse_source(source);

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.program.body.len(), 4);
    assert!(matches!(
        parsed.program.body[0].kind,
        StatementKind::Import(_)
    ));
    assert!(matches!(
        parsed.program.body[1].kind,
        StatementKind::Export(_)
    ));
    assert!(matches!(
        parsed.program.body[2].kind,
        StatementKind::Class(_)
    ));

    let StatementKind::Variable(variable) = &parsed.program.body[3].kind else {
        panic!("expected variable");
    };
    assert!(matches!(
        variable.ty.as_ref().unwrap().kind,
        TypeKind::Array(_)
    ));
    assert!(matches!(
        variable.initializer.as_ref().unwrap().kind,
        ExpressionKind::Array(_)
    ));
}

#[test]
fn recovers_from_common_syntax_errors_and_keeps_later_statements() {
    let source = r#"
const broken: number = ;
const good: number = 42;
"#;

    let parsed = parse_source(source);

    assert!(!parsed.diagnostics.is_empty());
    assert_eq!(parsed.program.body.len(), 2);
    assert!(matches!(
        parsed.program.body[0].kind,
        StatementKind::Variable(_)
    ));

    let StatementKind::Variable(variable) = &parsed.program.body[1].kind else {
        panic!("expected recovered variable");
    };
    assert_eq!(variable.name, "good");
    assert_eq!(variable.ty.as_ref().unwrap().kind, TypeKind::Number);
}

#[test]
fn parses_valid_fixture_corpus_without_diagnostics() {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("parser")
        .join("valid");

    for entry in std::fs::read_dir(&fixture_root).expect("parser fixture directory should exist") {
        let path = entry.expect("fixture entry should be readable").path();
        if !path.is_file() {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("fixture should be readable");
        let parsed = parse_source(&source);
        assert!(
            parsed.diagnostics.is_empty(),
            "{} diagnostics: {:?}",
            path.display(),
            parsed.diagnostics
        );
        assert!(
            !parsed.program.body.is_empty(),
            "{} should parse statements",
            path.display()
        );
    }
}
