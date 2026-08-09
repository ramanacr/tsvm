#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    fmt::Write,
    os::raw::c_int,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
};

use tsvm_interop::HostEnvironment;
use tsvm_interpreter::{execute_source, ExecuteError, ExecutionOutput, Value};
use tsvm_script_loader::{PageScriptSession, ScriptPolicy};

pub const TSVM_ABI_VERSION: u32 = 2;
pub const TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT: c_int = 0;
pub const TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT: c_int = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Status {
    Ok = 0,
    InvalidArgument = 1,
    InvalidUtf8 = 2,
    CompileError = 3,
    VerifyError = 4,
    RuntimeError = 5,
    InternalError = 6,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub entries: usize,
}

impl Status {
    fn from_execute_error(error: &ExecuteError) -> Self {
        match error {
            ExecuteError::Module(_) | ExecuteError::Compile(_) => Self::CompileError,
            ExecuteError::Verify(_) => Self::VerifyError,
            ExecuteError::Interop(_) | ExecuteError::Runtime(_) => Self::RuntimeError,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::InvalidArgument => "invalid_argument",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::CompileError => "compile_error",
            Self::VerifyError => "verify_error",
            Self::RuntimeError => "runtime_error",
            Self::InternalError => "internal_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CApiResult {
    pub status: Status,
    pub json: String,
}

impl CApiResult {
    fn success(output: &ExecutionOutput) -> Self {
        Self {
            status: Status::Ok,
            json: render_success(output),
        }
    }

    fn error(status: Status, message: impl AsRef<str>) -> Self {
        Self {
            status,
            json: format!(
                "{{\"generated_javascript\":false,\"status\":\"{}\",\"message\":{}}}",
                status.name(),
                json_string(message.as_ref())
            ),
        }
    }
}

pub struct TsvmResult {
    json: Vec<u8>,
}

pub struct TsvmPageSession {
    session: PageScriptSession,
}

pub fn execute_utf8(source: &[u8]) -> CApiResult {
    let source = match std::str::from_utf8(source) {
        Ok(source) => source,
        Err(error) => return CApiResult::error(Status::InvalidUtf8, error.to_string()),
    };

    match execute_source(source) {
        Ok(output) => CApiResult::success(&output),
        Err(error) => CApiResult::error(Status::from_execute_error(&error), format!("{error:?}")),
    }
}

/// Executes a length-delimited UTF-8 TypeScript source buffer through TSVM.
///
/// # Safety
///
/// `source` must point to `source_len` readable bytes when `source_len` is
/// non-zero. `out_result` must point to writable memory for one result pointer.
/// A non-null result returned through `out_result` must be freed exactly once
/// with [`tsvm_result_free`].
#[no_mangle]
pub unsafe extern "C" fn tsvm_execute_utf8(
    source: *const u8,
    source_len: usize,
    out_result: *mut *mut TsvmResult,
) -> Status {
    if out_result.is_null() {
        return Status::InvalidArgument;
    }

    unsafe { out_result.write(ptr::null_mut()) };

    let result = catch_unwind(AssertUnwindSafe(|| {
        let source = unsafe { source_from_raw(source, source_len) }?;
        Ok::<_, Status>(execute_utf8(source))
    }));

    match result {
        Ok(Ok(result)) => unsafe { write_result(out_result, result) },
        Ok(Err(status)) => status,
        Err(_) => unsafe {
            write_result(
                out_result,
                CApiResult::error(Status::InternalError, "TSVM C ABI execution panicked"),
            )
        },
    }
}

/// Creates a bounded page-owned preparation cache for inline TypeScript source.
///
/// # Safety
///
/// `out_session` must point to writable memory for one session pointer. A
/// non-null session returned through `out_session` must be released exactly
/// once with [`tsvm_page_session_free`]. Calls for the same session must be
/// serialized by the caller.
#[no_mangle]
pub unsafe extern "C" fn tsvm_page_session_create(
    cache_capacity: usize,
    out_session: *mut *mut TsvmPageSession,
) -> Status {
    if out_session.is_null() {
        return Status::InvalidArgument;
    }

    unsafe { out_session.write(ptr::null_mut()) };
    let session = catch_unwind(AssertUnwindSafe(|| PageScriptSession::new(cache_capacity)));
    match session {
        Ok(Ok(session)) => {
            unsafe { out_session.write(Box::into_raw(Box::new(TsvmPageSession { session }))) };
            Status::Ok
        }
        Ok(Err(_)) => Status::InvalidArgument,
        Err(_) => Status::InternalError,
    }
}

/// Executes inline UTF-8 TypeScript against a persistent page session.
///
/// # Safety
///
/// `session` must be a live pointer returned by [`tsvm_page_session_create`].
/// `source` must point to `source_len` readable bytes when `source_len` is
/// non-zero. `out_result` must point to writable memory for one result pointer.
/// A non-null result returned through `out_result` must be freed exactly once
/// with [`tsvm_result_free`]. Calls for the same session must be serialized.
#[no_mangle]
pub unsafe extern "C" fn tsvm_page_session_execute_utf8(
    session: *mut TsvmPageSession,
    source: *const u8,
    source_len: usize,
    policy: c_int,
    out_result: *mut *mut TsvmResult,
) -> Status {
    if session.is_null() || out_result.is_null() {
        return Status::InvalidArgument;
    }

    unsafe { out_result.write(ptr::null_mut()) };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let source = unsafe { source_from_raw(source, source_len) }?;
        let policy = script_policy_from_raw(policy)?;
        let source = match std::str::from_utf8(source) {
            Ok(source) => source,
            Err(error) => {
                return Ok::<_, Status>(CApiResult::error(Status::InvalidUtf8, error.to_string()))
            }
        };
        let page = unsafe { &mut *session };
        Ok(execute_page_session_utf8(page, source, policy))
    }));

    match result {
        Ok(Ok(result)) => unsafe { write_result(out_result, result) },
        Ok(Err(status)) => status,
        Err(_) => unsafe {
            write_result(
                out_result,
                CApiResult::error(
                    Status::InternalError,
                    "TSVM page-session execution panicked",
                ),
            )
        },
    }
}

/// Copies cache counters from a persistent page session.
///
/// # Safety
///
/// `session` must be a live pointer returned by [`tsvm_page_session_create`]
/// and `out_stats` must point to writable memory for one [`CacheStats`] value.
#[no_mangle]
pub unsafe extern "C" fn tsvm_page_session_cache_stats(
    session: *const TsvmPageSession,
    out_stats: *mut CacheStats,
) -> Status {
    if session.is_null() || out_stats.is_null() {
        return Status::InvalidArgument;
    }

    unsafe { out_stats.write(CacheStats::default()) };
    match catch_unwind(AssertUnwindSafe(|| {
        let stats = unsafe { (&*session).session.cache_stats() };
        CacheStats {
            hits: stats.hits,
            misses: stats.misses,
            evictions: stats.evictions,
            entries: stats.entries,
        }
    })) {
        Ok(stats) => {
            unsafe { out_stats.write(stats) };
            Status::Ok
        }
        Err(_) => Status::InternalError,
    }
}

/// Releases a page session returned through [`tsvm_page_session_create`].
///
/// # Safety
///
/// `session` must be null or a live session pointer returned by this ABI. Every
/// non-null pointer may be released exactly once.
#[no_mangle]
pub unsafe extern "C" fn tsvm_page_session_free(session: *mut TsvmPageSession) {
    if !session.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            drop(unsafe { Box::from_raw(session) });
        }));
    }
}

/// Returns a borrowed JSON byte view owned by `result`.
///
/// # Safety
///
/// `result` must be a live pointer returned by [`tsvm_execute_utf8`] and
/// `out_len` must be writable. The returned bytes are valid until
/// [`tsvm_result_free`] is called for the same result.
#[no_mangle]
pub unsafe extern "C" fn tsvm_result_json(
    result: *const TsvmResult,
    out_len: *mut usize,
) -> *const u8 {
    if result.is_null() || out_len.is_null() {
        return ptr::null();
    }

    let result = unsafe { &*result };
    unsafe { out_len.write(result.json.len()) };
    result.json.as_ptr()
}

/// Releases a result returned through [`tsvm_execute_utf8`].
///
/// # Safety
///
/// `result` must be null or a live result pointer returned by this ABI. Every
/// non-null pointer may be released exactly once.
#[no_mangle]
pub unsafe extern "C" fn tsvm_result_free(result: *mut TsvmResult) {
    if !result.is_null() {
        drop(unsafe { Box::from_raw(result) });
    }
}

#[no_mangle]
pub extern "C" fn tsvm_abi_version() -> u32 {
    TSVM_ABI_VERSION
}

unsafe fn source_from_raw<'source>(
    source: *const u8,
    source_len: usize,
) -> Result<&'source [u8], Status> {
    if source_len == 0 {
        return Ok(&[]);
    }
    if source.is_null() {
        return Err(Status::InvalidArgument);
    }

    Ok(unsafe { slice::from_raw_parts(source, source_len) })
}

fn script_policy_from_raw(policy: c_int) -> Result<ScriptPolicy, Status> {
    match policy {
        TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT => Ok(ScriptPolicy::default()),
        TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT => Ok(ScriptPolicy {
            allow_typescript: false,
        }),
        _ => Err(Status::InvalidArgument),
    }
}

fn execute_page_session_utf8(
    session: &mut TsvmPageSession,
    source: &str,
    policy: ScriptPolicy,
) -> CApiResult {
    match session
        .session
        .execute_inline_typescript(source, &HostEnvironment::new(), policy)
    {
        Ok(output) => CApiResult::success(&output),
        Err(error) => CApiResult::error(
            error
                .source
                .as_ref()
                .map_or(Status::RuntimeError, Status::from_execute_error),
            &error.message,
        ),
    }
}

unsafe fn write_result(out_result: *mut *mut TsvmResult, result: CApiResult) -> Status {
    let status = result.status;
    let result = Box::new(TsvmResult {
        json: result.json.into_bytes(),
    });
    unsafe { out_result.write(Box::into_raw(result)) };
    status
}

fn render_success(output: &ExecutionOutput) -> String {
    let console = output
        .console
        .iter()
        .map(render_value)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"generated_javascript\":false,\"status\":\"ok\",\"console\":[{console}],\"return_value\":{},\"heap\":{{\"live_objects\":{},\"allocated_slots\":{}}}}}",
        render_value(&output.return_value),
        output.heap.live_objects,
        output.heap.allocated_slots,
    )
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Number(value) if value.is_finite() => {
            format!("{{\"kind\":\"number\",\"value\":{value}}}")
        }
        Value::Number(value) => format!(
            "{{\"kind\":\"number\",\"value\":null,\"non_finite\":{}}}",
            json_string(&value.to_string())
        ),
        Value::String(value) => format!("{{\"kind\":\"string\",\"value\":{}}}", json_string(value)),
        Value::Boolean(value) => format!("{{\"kind\":\"boolean\",\"value\":{value}}}"),
        Value::Null => "{\"kind\":\"null\"}".into(),
        Value::Undefined => "{\"kind\":\"undefined\"}".into(),
        Value::Object(values) => {
            let fields = values
                .iter()
                .map(|(name, value)| format!("{}:{}", json_string(name), render_value(value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"kind\":\"object\",\"value\":{{{fields}}}}}")
        }
        Value::Array(values) => {
            let values = values
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"kind\":\"array\",\"value\":[{values}]}}")
        }
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}
