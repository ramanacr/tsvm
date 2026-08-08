use tsvm_ir::{lower_source, IrBinaryOp, IrInstructionKind, IrType};

#[test]
fn lowers_initial_demo_to_entry_and_function_ir() {
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

    let lowered = lower_source(source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let ir = lowered.ir.expect("valid source should lower");
    assert_eq!(ir.functions.len(), 1);
    assert_eq!(ir.functions[0].name, "credit");
    assert_eq!(ir.functions[0].return_type, IrType::Number);
    assert_eq!(ir.functions[0].params[0].name, "account");
    assert_eq!(
        ir.functions[0].params[0].ty,
        IrType::Named("Account".into())
    );
    assert!(ir.source_span.is_some());

    let function_instrs = &ir.functions[0].blocks[0].instructions;
    assert!(function_instrs.iter().any(|instruction| {
        matches!(
            instruction.kind,
            IrInstructionKind::StoreMember {
                property: ref name,
                ..
            } if name == "balance"
        )
    }));
    assert!(function_instrs
        .iter()
        .any(|instruction| matches!(instruction.kind, IrInstructionKind::Return(Some(_)))));

    let entry_instrs = &ir.entry.blocks[0].instructions;
    assert!(entry_instrs.iter().any(|instruction| {
        matches!(instruction.kind, IrInstructionKind::Call { ref callee, .. } if callee == "credit")
    }));
    assert!(entry_instrs.iter().any(|instruction| {
        matches!(instruction.kind, IrInstructionKind::Call { ref callee, .. } if callee == "console.log")
    }));
}

#[test]
fn preserves_source_spans_on_lowered_instructions() {
    let lowered = lower_source("const answer: number = 40 + 2;");
    let ir = lowered.ir.expect("valid source should lower");

    let binary = ir.entry.blocks[0]
        .instructions
        .iter()
        .find(|instruction| {
            matches!(
                instruction.kind,
                IrInstructionKind::Binary {
                    op: IrBinaryOp::Add,
                    ..
                }
            )
        })
        .expect("binary add instruction should exist");

    assert_eq!(binary.ty, IrType::Number);
    assert_eq!(binary.source_span.start.column, 24);
    assert_eq!(binary.source_span.end.column, 30);
}

#[test]
fn lowers_if_else_into_multiple_blocks() {
    let lowered = lower_source(
        r#"
let x: number = 1;
if (x > 0) {
  x += 1;
} else {
  x += 2;
}
"#,
    );
    let ir = lowered.ir.expect("valid source should lower");

    assert!(ir.entry.blocks.len() >= 4);
    assert!(ir.entry.blocks[0]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction.kind, IrInstructionKind::Branch { .. })));
}

#[test]
fn does_not_lower_semantically_invalid_source() {
    let lowered = lower_source(
        r#"
interface Account {
  id: number;
  balance: number;
}

const account: Account = { id: 1 };
"#,
    );

    assert!(lowered.ir.is_none());
    assert!(!lowered.diagnostics.is_empty());
}

#[test]
fn lowers_valid_ir_fixture_corpus() {
    let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("ir")
        .join("valid");

    for entry in std::fs::read_dir(&fixture_root).expect("IR fixture directory should exist") {
        let path = entry.expect("fixture entry should be readable").path();
        if !path.is_file() {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("fixture should be readable");
        let lowered = lower_source(&source);
        assert!(
            lowered.diagnostics.is_empty(),
            "{} diagnostics: {:?}",
            path.display(),
            lowered.diagnostics
        );
        assert!(
            lowered.ir.is_some(),
            "{} should lower to IR",
            path.display()
        );
    }
}
