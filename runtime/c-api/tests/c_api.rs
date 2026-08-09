use std::{ptr, slice};

use tsvm_c_api::{
    tsvm_execute_utf8, tsvm_page_session_cache_stats, tsvm_page_session_create,
    tsvm_page_session_execute_utf8, tsvm_page_session_free, tsvm_result_free, tsvm_result_json,
    CacheStats, Status, TsvmResult, TSVM_ABI_VERSION, TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
    TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT,
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
    assert_eq!(TSVM_ABI_VERSION, 2);
}

#[test]
fn page_session_creation_rejects_invalid_output_and_zero_capacity() {
    // Removing either creation guard would make this test fail.
    let null_output = unsafe { tsvm_page_session_create(1, ptr::null_mut()) };
    assert_eq!(null_output, Status::InvalidArgument);

    let mut session = ptr::null_mut();
    let zero_capacity = unsafe { tsvm_page_session_create(0, &mut session) };
    assert_eq!(zero_capacity, Status::InvalidArgument);
    assert!(session.is_null());
}

#[test]
fn page_session_reuses_preparation_for_identical_inline_source() {
    // Bypassing the page-owned preparation cache would make the counters differ.
    let mut session = ptr::null_mut();
    assert_eq!(
        unsafe { tsvm_page_session_create(1, &mut session) },
        Status::Ok
    );

    let source = b"console.log(150);";
    for _ in 0..2 {
        let mut result = ptr::null_mut();
        let status = unsafe {
            tsvm_page_session_execute_utf8(
                session,
                source.as_ptr(),
                source.len(),
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                &mut result,
            )
        };

        assert_eq!(status, Status::Ok);
        let json = unsafe { result_json(result) };
        assert!(json.contains("\"generated_javascript\":false"));
        assert!(json.contains("\"kind\":\"number\",\"value\":150"));
        unsafe { tsvm_result_free(result) };
    }

    let mut stats = CacheStats::default();
    assert_eq!(
        unsafe { tsvm_page_session_cache_stats(session, &mut stats) },
        Status::Ok
    );
    assert_eq!(
        (stats.hits, stats.misses, stats.evictions, stats.entries),
        (1, 1, 0, 1)
    );
    unsafe { tsvm_page_session_free(session) };
}

#[test]
fn blocked_page_session_execution_leaves_existing_cache_stats_unchanged() {
    // Moving policy validation after cache lookup would make this test fail.
    let mut session = ptr::null_mut();
    assert_eq!(
        unsafe { tsvm_page_session_create(1, &mut session) },
        Status::Ok
    );
    let source = b"console.log(150);";
    let mut prepared_result = ptr::null_mut();
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session,
                source.as_ptr(),
                source.len(),
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                &mut prepared_result,
            )
        },
        Status::Ok
    );
    unsafe { tsvm_result_free(prepared_result) };

    let mut blocked_result = ptr::null_mut();
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session,
                source.as_ptr(),
                source.len(),
                TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT,
                &mut blocked_result,
            )
        },
        Status::RuntimeError
    );
    assert!(unsafe { result_json(blocked_result) }.contains("\"status\":\"runtime_error\""));
    unsafe { tsvm_result_free(blocked_result) };

    let mut stats = CacheStats::default();
    assert_eq!(
        unsafe { tsvm_page_session_cache_stats(session, &mut stats) },
        Status::Ok
    );
    assert_eq!(
        (stats.hits, stats.misses, stats.evictions, stats.entries),
        (0, 1, 0, 1)
    );
    unsafe { tsvm_page_session_free(session) };
}

#[test]
fn page_session_preserves_error_result_and_pointer_contracts() {
    // Removing a raw pointer, policy, or UTF-8 guard would make an assertion fail.
    let source = b"console.log(1);";
    let mut result = ptr::null_mut();
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                ptr::null_mut(),
                source.as_ptr(),
                source.len(),
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                &mut result,
            )
        },
        Status::InvalidArgument
    );
    assert!(result.is_null());

    let mut session = ptr::null_mut();
    assert_eq!(
        unsafe { tsvm_page_session_create(1, &mut session) },
        Status::Ok
    );
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session,
                ptr::null(),
                1,
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                &mut result,
            )
        },
        Status::InvalidArgument
    );
    assert!(result.is_null());
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(session, source.as_ptr(), source.len(), 99, &mut result)
        },
        Status::InvalidArgument
    );
    assert!(result.is_null());
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session,
                source.as_ptr(),
                source.len(),
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                ptr::null_mut(),
            )
        },
        Status::InvalidArgument
    );

    let invalid_utf8 = [0xff_u8];
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session,
                invalid_utf8.as_ptr(),
                invalid_utf8.len(),
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                &mut result,
            )
        },
        Status::InvalidUtf8
    );
    assert!(unsafe { result_json(result) }.contains("\"status\":\"invalid_utf8\""));
    unsafe { tsvm_result_free(result) };

    let compile_error = b"const answer: number = \"wrong\";";
    result = ptr::null_mut();
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session,
                compile_error.as_ptr(),
                compile_error.len(),
                TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
                &mut result,
            )
        },
        Status::CompileError
    );
    assert!(unsafe { result_json(result) }.contains("\"status\":\"compile_error\""));
    unsafe { tsvm_result_free(result) };

    assert_eq!(
        unsafe { tsvm_page_session_cache_stats(ptr::null(), ptr::null_mut()) },
        Status::InvalidArgument
    );
    assert_eq!(
        unsafe { tsvm_page_session_cache_stats(session, ptr::null_mut()) },
        Status::InvalidArgument
    );
    unsafe { tsvm_page_session_free(session) };
}
