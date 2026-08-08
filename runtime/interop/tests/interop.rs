use tsvm_interop::{HostEnvironment, InteropError, InteropValue};

fn add(args: &[InteropValue]) -> Result<InteropValue, InteropError> {
    match args {
        [InteropValue::Number(left), InteropValue::Number(right)] => {
            Ok(InteropValue::Number(left + right))
        }
        _ => Err(InteropError::new("expected two numbers")),
    }
}

#[test]
fn host_environment_dispatches_registered_functions() {
    let host = HostEnvironment::new().with_function("host.add", add);

    let value = host
        .call(
            "host.add",
            &[InteropValue::Number(20.0), InteropValue::Number(22.0)],
        )
        .expect("function should exist")
        .expect("function should succeed");

    assert_eq!(value, InteropValue::Number(42.0));
}

#[test]
fn host_environment_reports_absent_functions_without_side_effects() {
    let host = HostEnvironment::new();

    assert!(host.call("host.missing", &[]).is_none());
    assert!(!host.contains_function("host.missing"));
}
