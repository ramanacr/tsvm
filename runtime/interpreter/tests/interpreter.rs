use tsvm_bytecode::{compile_source, decode_module, encode_module, Opcode};
use tsvm_interpreter::{execute_module, execute_source, ExecuteError, Value};

fn initial_demo_source() -> &'static str {
    r#"
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
"#
}

#[test]
fn executes_initial_demo_through_verified_bytecode() {
    let output = execute_source(initial_demo_source()).expect("demo should execute");

    assert_eq!(output.console, vec![Value::Number(150.0)]);
    assert_eq!(output.return_value, Value::Undefined);
}

#[test]
fn executes_decoded_bytecode_module() {
    let module = compile_source(initial_demo_source())
        .module
        .expect("demo should compile");
    let decoded = decode_module(&encode_module(&module)).expect("bytecode should decode");

    let output = execute_module(&decoded).expect("decoded module should execute");

    assert_eq!(output.console, vec![Value::Number(150.0)]);
}

#[test]
fn refuses_unverified_bytecode_before_execution() {
    let mut module = compile_source("const answer: number = 42;")
        .module
        .expect("source should compile");
    module.functions[0].blocks[0].instructions[0].opcode = Opcode::Jump;
    module.functions[0].blocks[0].instructions[0].operands = vec![999];

    let err = execute_module(&module).expect_err("invalid module must not execute");

    assert!(matches!(err, ExecuteError::Verify(_)));
}

#[test]
fn reports_semantic_diagnostics_without_bytecode_execution() {
    let err = execute_source("const answer: number = \"bad\";")
        .expect_err("invalid source should not execute");

    assert!(matches!(err, ExecuteError::Compile(_)));
}

#[test]
fn executes_valid_interpreter_fixture_corpus() {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("interpreter")
        .join("valid");

    for entry in std::fs::read_dir(&fixture_root).expect("interpreter fixtures should exist") {
        let path = entry.expect("fixture entry should be readable").path();
        if !path.is_file() {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("fixture should be readable");
        let output = execute_source(&source).expect("fixture should execute");
        assert_eq!(
            output.console,
            vec![Value::Number(150.0)],
            "{} output mismatch",
            path.display()
        );
    }
}
