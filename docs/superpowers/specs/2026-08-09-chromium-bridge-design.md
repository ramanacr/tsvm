# TSVM Chromium Bridge Design

## Purpose

This milestone creates the native boundary required to embed TSVM in a Chromium
renderer. It does not claim to be a browser embed: no Chromium checkout is
available locally, and the host has 88 GB free while Chromium's Windows build
instructions require at least 100 GB.

The bridge preserves the project invariant:

```text
TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM
```

No bridge API may generate or execute JavaScript for TypeScript input.

## Decision

Create a dependency-free Rust C ABI in `runtime/c-api` and a small C++
renderer-facing wrapper in `browser/chromium`. The C ABI is the only crate
allowed to use narrowly scoped `unsafe` operations, solely to validate and
convert C pointers and to manage opaque result ownership. All TSVM execution
continues through `tsvm_interpreter::execute_source`.

The initial ABI is synchronous and source-only. Module maps, host callbacks,
DOM capabilities, async work, and V8 values are intentionally excluded until
the real renderer integration can supply their browser-owned lifetimes and
security policy.

## Public C ABI

`runtime/c-api/include/tsvm_c_api.h` defines ABI version 1.

```c
typedef enum tsvm_status {
  TSVM_STATUS_OK = 0,
  TSVM_STATUS_INVALID_ARGUMENT = 1,
  TSVM_STATUS_INVALID_UTF8 = 2,
  TSVM_STATUS_COMPILE_ERROR = 3,
  TSVM_STATUS_VERIFY_ERROR = 4,
  TSVM_STATUS_RUNTIME_ERROR = 5,
  TSVM_STATUS_INTERNAL_ERROR = 6,
} tsvm_status;

typedef struct tsvm_result tsvm_result;

tsvm_status tsvm_execute_utf8(const unsigned char* source,
                              size_t source_len,
                              tsvm_result** out_result);
const unsigned char* tsvm_result_json(const tsvm_result* result,
                                      size_t* out_len);
void tsvm_result_free(tsvm_result* result);
unsigned int tsvm_abi_version(void);
```

`tsvm_execute_utf8` accepts a non-null buffer or a null pointer only when
`source_len` is zero. It initializes a non-null `out_result` to null before
doing work. For valid calls it returns an owned result for success and for every
recoverable error; callers must release it exactly once with
`tsvm_result_free`. A null `out_result`, a null non-empty source, and a null
length-output pointer return `TSVM_STATUS_INVALID_ARGUMENT` without panic.

`tsvm_result_json` returns a borrowed UTF-8 byte view that remains valid until
the matching free call. The caller never frees the returned bytes separately.
The API does not accept arbitrary `tsvm_result` pointers as safely valid: C++
callers must retain only pointers received from this ABI and free each at most
once, the ordinary ownership contract for opaque C handles.

## Result Envelope

The result payload is deterministic JSON, serialized without a JavaScript
runtime or serializer dependency. Successful output is a tagged representation
of `ExecutionOutput`:

```json
{
  "generated_javascript": false,
  "console": [{"kind":"number","value":150}],
  "return_value": {"kind":"undefined"},
  "heap": {"live_objects":0,"allocated_slots":1}
}
```

Every value is tagged so `undefined`, `null`, primitives, arrays, and objects
remain unambiguous. Errors return an envelope with a stable status name and a
human-readable message. Diagnostics are intentionally text in ABI v1; adding
structured span fields later requires a versioned extension rather than a
breaking enum change.

## Rust Boundary Design

`runtime/c-api` builds both `staticlib` and `cdylib`, allowing Chromium GN
targets to link the static archive while standalone consumers can load a DLL.
Its public Rust API remains small and testable:

```rust
pub fn execute_utf8(source: &[u8]) -> CApiResult;
pub struct CApiResult {
    pub status: Status,
    pub json: String,
}
```

The safe function validates UTF-8, calls `execute_source`, maps
`ExecuteError` into stable statuses, and formats the result. `extern "C"`
exports are thin adapters around this safe core. They use `catch_unwind` so a
panic never crosses the C++ boundary. The crate will not inherit the workspace
`unsafe_code = forbid` lint; instead it locally permits unsafe code only for
the FFI adapter module. The rest of the crate stays safe Rust.

## C++ Renderer Wrapper

`browser/chromium/tsvm_renderer_bridge.h` and `.cc` define a dependency-light
RAII adapter:

```cpp
struct TsvmExecutionResult {
  TsvmStatus status;
  std::string json;
};

TsvmExecutionResult ExecuteTsvmSource(std::string_view source);
```

It converts a renderer-owned script source into the byte view expected by the C
API, copies the borrowed JSON before freeing the opaque result, and never
passes V8, DOM, or browser-process pointers into Rust. A future Blink hook must
call it only after normal script policy and origin checks, then route console,
DOM, fetch, and diagnostics through browser-owned capability bindings.

## Security And Error Handling

- TSVM remains renderer-process code; this bridge grants no filesystem,
  network, IPC, V8, or DOM capability.
- Input is length-delimited UTF-8, never null-terminated text.
- The bridge maps compile, verifier, and runtime failures explicitly and
  converts unexpected panics to `TSVM_STATUS_INTERNAL_ERROR`.
- Opaque result allocation and release stay on the Rust side; there is no
  cross-allocator ownership.
- Result JSON explicitly records `generated_javascript: false`, making the
  invariant observable to browser smoke tests.

## Test Strategy

Tests are written before implementation and cover real ABI behavior:

1. The initial account TypeScript demo returns `TSVM_STATUS_OK`, reports
   `generated_javascript: false`, and contains console value `150`.
2. A malformed UTF-8 buffer returns `TSVM_STATUS_INVALID_UTF8` with a releasable
   error result.
3. A semantic error returns `TSVM_STATUS_COMPILE_ERROR` with diagnostics.
4. A null non-empty source and null output arguments return
   `TSVM_STATUS_INVALID_ARGUMENT` without unwinding.
5. The C++ wrapper compiles with a standard C++20 compiler and copies the
   payload before releasing its result handle.

Rust unit and integration tests exercise the C-exported functions directly.
CI gains a C++ syntax-only check for the renderer wrapper, while Rust tests,
formatting, and Clippy continue to cover the whole workspace.

## Delivery And Follow-Up

This milestone adds the ABI, C++ wrapper, tests, API documentation, and a
roadmap update. It is committed and pushed as one focused milestone.

The following milestone is an actual Chromium checkout and renderer integration
once at least 100 GB of free NTFS disk is available. That work will use the
bridge static library, add a Blink `text/typescript` dispatch hook, and prove
the browser smoke path without generated JavaScript.
