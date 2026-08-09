use std::{ptr, slice};

use tsvm_c_api::{
    tsvm_execute_utf8, tsvm_result_free, tsvm_result_json, Status, TsvmResult, TSVM_ABI_VERSION,
};

unsafe fn result_json(result: *const TsvmResult) -> String {
    let mut length = 0_usize;
    let bytes = unsafe { tsvm_result_json(result, &mut length) };
    assert!(!bytes.is_null(), "valid results expose a JSON payload");
    let payload = unsafe { slice::from_raw_parts(bytes, length) };
    String::from_utf8(payload.to_vec()).expect("result payload is valid UTF-8")
}

#[test]
fn exported_abi_runs_typescript_without_generating_javascript() {
    // Removing the C ABI execution path would make this assertion fail.
    let source = b"console.log(150);";
    let mut result = ptr::null_mut();

    let status = unsafe { tsvm_execute_utf8(source.as_ptr(), source.len(), &mut result) };

    assert_eq!(status, Status::Ok);
    assert!(!result.is_null());
    let json = unsafe { result_json(result) };
    assert!(json.contains("\"generated_javascript\":false"));
    assert!(json.contains("\"kind\":\"number\",\"value\":150"));
    unsafe { tsvm_result_free(result) };
}

#[test]
fn exported_abi_reports_invalid_utf8_with_an_owned_error_result() {
    // Accepting malformed UTF-8 would make this test fail.
    let source = [0xff_u8];
    let mut result = ptr::null_mut();

    let status = unsafe { tsvm_execute_utf8(source.as_ptr(), source.len(), &mut result) };

    assert_eq!(status, Status::InvalidUtf8);
    assert!(!result.is_null());
    let json = unsafe { result_json(result) };
    assert!(json.contains("\"status\":\"invalid_utf8\""));
    unsafe { tsvm_result_free(result) };
}

#[test]
fn exported_abi_reports_semantic_diagnostics_as_compile_errors() {
    // Collapsing compiler failures into runtime errors would make this test fail.
    let source = b"const answer: number = \"wrong\";";
    let mut result = ptr::null_mut();

    let status = unsafe { tsvm_execute_utf8(source.as_ptr(), source.len(), &mut result) };

    assert_eq!(status, Status::CompileError);
    assert!(!result.is_null());
    let json = unsafe { result_json(result) };
    assert!(json.contains("\"status\":\"compile_error\""));
    unsafe { tsvm_result_free(result) };
}

#[test]
fn exported_abi_rejects_invalid_pointer_contracts_without_a_result() {
    // Omitting pointer validation would make at least one assertion fail.
    let mut result = ptr::null_mut();
    let source = b"console.log(1);";

    let null_source = unsafe { tsvm_execute_utf8(ptr::null(), 1, &mut result) };
    assert_eq!(null_source, Status::InvalidArgument);
    assert!(result.is_null());

    let null_output = unsafe { tsvm_execute_utf8(source.as_ptr(), source.len(), ptr::null_mut()) };
    assert_eq!(null_output, Status::InvalidArgument);
}

#[test]
fn exported_abi_reports_its_first_version() {
    // Changing the public ABI version requires this test to be updated deliberately.
    assert_eq!(TSVM_ABI_VERSION, 1);
}
