# C ABI Bridge

`runtime/c-api` is the native boundary for a future Chromium renderer embed. It
executes length-delimited TypeScript source through the normal TSVM pipeline:

```text
TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM
```

It never generates or executes JavaScript for TypeScript input.

## Build Artifacts

The crate builds `staticlib`, `cdylib`, and `rlib` artifacts. Chromium GN
targets should link the static library; the dynamic library is available for
standalone embedding experiments.

```sh
cargo build -p tsvm-c-api
```

The public C header is
[`runtime/c-api/include/tsvm_c_api.h`](../runtime/c-api/include/tsvm_c_api.h).
The ABI version is `1`.

## API

```c
tsvm_status tsvm_execute_utf8(const unsigned char* source,
                              size_t source_len,
                              tsvm_result** out_result);
const unsigned char* tsvm_result_json(const tsvm_result* result,
                                      size_t* out_len);
void tsvm_result_free(tsvm_result* result);
uint32_t tsvm_abi_version(void);
```

`tsvm_execute_utf8` accepts a UTF-8 byte buffer. `source` may be null only when
`source_len` is zero. A valid `out_result` is reset to null before execution;
successes and recoverable source failures return an owned `tsvm_result` through
that pointer. Null output pointers and null non-empty sources return
`TSVM_STATUS_INVALID_ARGUMENT`.

The result object is opaque and owned by Rust. `tsvm_result_json` returns a
borrowed UTF-8 byte view valid until `tsvm_result_free` is called. Copy those
bytes before freeing the result and never free the bytes separately. A non-null
result must be released exactly once with `tsvm_result_free`.

Passing an arbitrary result pointer or freeing one twice violates the C API
contract. This is the usual opaque-handle rule and avoids cross-allocator
ownership between C++ and Rust.

## Result Envelope

Every result is deterministic JSON. Successful execution includes a visible
proof that TypeScript did not travel through JavaScript:

```json
{
  "generated_javascript": false,
  "status": "ok",
  "console": [{"kind":"number","value":150}],
  "return_value": {"kind":"undefined"},
  "heap": {"live_objects":0,"allocated_slots":1}
}
```

Values use tags so `undefined`, `null`, primitives, arrays, and objects remain
unambiguous. Invalid UTF-8, compile, verifier, and runtime failures return a
status and an error JSON envelope. Panics are caught at the ABI boundary and
reported as `TSVM_STATUS_INTERNAL_ERROR`; they never unwind into C++.

## Renderer Adapter

[`browser/chromium/tsvm_renderer_bridge.h`](../browser/chromium/tsvm_renderer_bridge.h)
provides the C++20 wrapper:

```cpp
const auto result = tsvm::chromium::ExecuteSource("console.log(150);");
```

It owns no V8, Blink, DOM, IPC, filesystem, or network capability. A future
Blink script hook must run normal script policy and origin checks before calling
this bridge, then map TSVM output to browser-owned console and capability
bindings. The bridge belongs in a renderer process, never a privileged browser
process.

On Windows, use the Visual Studio Build Tools developer shell for a local syntax
check:

```powershell
cmd /d /s /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 && cl /nologo /std:c++20 /Zs /I browser\chromium /I runtime\c-api\include browser\chromium\tsvm_renderer_bridge.cc browser\chromium\renderer_bridge_smoke.cc'
```

CI runs the equivalent C++20 syntax check on Ubuntu. A real Chromium checkout
still needs a Blink dispatch hook for `text/typescript`; allocate at least 100
GB free NTFS space, with 200 GB preferred for repeat builds and outputs.

When linking the Windows `staticlib` directly, also link the Rust runtime's
system dependencies:

```text
kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib
```

For example, the local bridge smoke links `target/debug/tsvm_c_api.lib` together
with those libraries and then executes a TypeScript `console.log(150)` program
through the C++ wrapper.
