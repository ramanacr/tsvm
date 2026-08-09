use tsvm_bytecode::{compile_source, decode_module, encode_module, Opcode};
use tsvm_interop::{HostEnvironment, InteropError, InteropValue};
use tsvm_interpreter::{
    execute_module, execute_module_graph, execute_source, CacheLookupStatus, ExecuteError,
    PreparedModule, PreparedModuleCache, PreparedModuleCacheError, Value,
};

fn host_add(args: &[InteropValue]) -> Result<InteropValue, InteropError> {
    match args {
        [InteropValue::Number(left), InteropValue::Number(right)] => {
            Ok(InteropValue::Number(left + right))
        }
        _ => Err(InteropError::new("expected two numbers")),
    }
}

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
    assert_eq!(output.heap.live_objects, 0);
    assert_eq!(output.heap.last_collection.collected, 1);
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
fn retains_console_objects_as_cross_boundary_roots() {
    let output = execute_source(
        r#"
const account = {
  id: 7,
  balance: 25
};

console.log(account);
"#,
    )
    .expect("object fixture should execute");

    let mut expected = std::collections::BTreeMap::new();
    expected.insert("balance".into(), Value::Number(25.0));
    expected.insert("id".into(), Value::Number(7.0));

    assert_eq!(output.console, vec![Value::Object(expected)]);
    assert_eq!(output.heap.live_objects, 1);
    assert_eq!(output.heap.last_collection.marked, 1);
    assert_eq!(output.heap.last_collection.collected, 0);
}

#[test]
fn executes_local_module_graph_without_javascript_generation() {
    let sources = std::collections::BTreeMap::from([
        (
            "/app.ts".into(),
            r#"
import { Account, credit } from "./account.ts";

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
"#
            .into(),
        ),
        (
            "/account.ts".into(),
            r#"
export interface Account {
  id: number;
  balance: number;
}

export function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}
"#
            .into(),
        ),
    ]);

    let output = execute_module_graph("/app.ts", &sources).expect("module graph should execute");

    assert_eq!(output.console, vec![Value::Number(150.0)]);
}

#[test]
fn reports_module_diagnostics_before_compilation() {
    let sources = std::collections::BTreeMap::from([(
        "/app.ts".into(),
        r#"import { value } from "pkg"; console.log(value);"#.into(),
    )]);

    let err = execute_module_graph("/app.ts", &sources).expect_err("module diagnostics expected");

    assert!(matches!(err, ExecuteError::Module(_)));
}

#[test]
fn typescript_calls_registered_host_function() {
    let host = HostEnvironment::new().with_function("hostAdd", host_add);
    let output = tsvm_interpreter::execute_source_with_host(
        r#"
function hostAdd(a: number, b: number): number {
  return 0;
}

console.log(hostAdd(20, 22));
"#,
        &host,
    )
    .expect("host function should execute");

    assert_eq!(output.console, vec![Value::Number(42.0)]);
}

#[test]
fn host_calls_typescript_function_with_boundary_values() {
    let module = tsvm_interpreter::PreparedModule::from_source(
        r#"
function add(a: number, b: number): number {
  return a + b;
}
"#,
    )
    .expect("module should prepare");

    let value = module
        .call_function(
            "add",
            &[InteropValue::Number(20.0), InteropValue::Number(22.0)],
            &HostEnvironment::new(),
        )
        .expect("TS function should be callable");

    assert_eq!(value, InteropValue::Number(42.0));
}

#[test]
fn prepared_module_executes_verified_entry_with_host() {
    let prepared = PreparedModule::from_source(
        r#"
function hostAdd(a: number, b: number): number {
  return 0;
}

console.log(hostAdd(20, 22));
"#,
    )
    .expect("source should prepare");
    let host = HostEnvironment::new().with_function("hostAdd", host_add);

    let output = prepared
        .execute_with_host(&host)
        .expect("prepared entry should execute");

    assert_eq!(output.console, vec![Value::Number(42.0)]);
}

#[test]
fn prepared_entry_execution_uses_fresh_runtime_state() {
    let prepared = PreparedModule::from_source(
        "const state = { count: 1 }; state.count += 1; console.log(state.count);",
    )
    .expect("source should prepare");

    let first = prepared.execute().expect("first execution should succeed");
    let second = prepared.execute().expect("second execution should succeed");

    assert_eq!(first.console, vec![Value::Number(2.0)]);
    assert_eq!(second.console, vec![Value::Number(2.0)]);
    assert_eq!(first.heap, second.heap);
}

#[test]
fn prepared_module_refuses_invalid_source_before_execution() {
    let error = PreparedModule::from_source("const answer: number = \"bad\";")
        .expect_err("invalid source should not prepare");

    assert!(matches!(error, ExecuteError::Compile(_)));
}

#[test]
fn prepared_module_cache_reports_miss_then_hit_and_executes_cached_module() {
    let mut cache = PreparedModuleCache::new(2).expect("capacity should be valid");
    let first = cache
        .get_or_prepare("console.log(40 + 2);")
        .expect("first source should prepare");

    assert_eq!(first.status(), CacheLookupStatus::Miss);
    assert_eq!(
        first.module().execute().unwrap().console,
        vec![Value::Number(42.0)]
    );

    let second = cache
        .get_or_prepare("console.log(40 + 2);")
        .expect("cached source should execute");

    assert_eq!(second.status(), CacheLookupStatus::Hit);
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().misses, 1);
    assert_eq!(cache.stats().evictions, 0);
    assert_eq!(cache.stats().entries, 1);
}

#[test]
fn prepared_module_cache_rejects_zero_capacity() {
    assert!(matches!(
        PreparedModuleCache::new(0),
        Err(PreparedModuleCacheError::ZeroCapacity)
    ));
}

#[test]
fn prepared_module_cache_evicts_oldest_inserted_source_without_reordering_hits() {
    let mut cache = PreparedModuleCache::new(2).expect("capacity should be valid");
    cache
        .get_or_prepare("console.log(1);")
        .expect("first source should prepare");
    cache
        .get_or_prepare("console.log(2);")
        .expect("second source should prepare");
    assert_eq!(
        cache
            .get_or_prepare("console.log(1);")
            .expect("first source should hit")
            .status(),
        CacheLookupStatus::Hit
    );
    cache
        .get_or_prepare("console.log(3);")
        .expect("third source should prepare");

    assert_eq!(
        cache
            .get_or_prepare("console.log(1);")
            .expect("evicted source should prepare again")
            .status(),
        CacheLookupStatus::Miss
    );
    assert_eq!(cache.stats().evictions, 2);
}

#[test]
fn prepared_module_cache_does_not_insert_invalid_source() {
    let mut cache = PreparedModuleCache::new(1).expect("capacity should be valid");

    for _ in 0..2 {
        assert!(matches!(
            cache.get_or_prepare("const answer: number = \"bad\";"),
            Err(ExecuteError::Compile(_))
        ));
    }

    assert_eq!(cache.stats().misses, 2);
    assert_eq!(cache.stats().entries, 0);
}

#[test]
fn host_errors_are_reported_at_the_interop_boundary() {
    fn fail(_args: &[InteropValue]) -> Result<InteropValue, InteropError> {
        Err(InteropError::new("host failure"))
    }

    let host = HostEnvironment::new().with_function("hostFail", fail);
    let err = tsvm_interpreter::execute_source_with_host(
        r#"
function hostFail(): number {
  return 0;
}

console.log(hostFail());
"#,
        &host,
    )
    .expect_err("host failure should cross boundary as interop error");

    assert!(matches!(err, ExecuteError::Interop(_)));
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
