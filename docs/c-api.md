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
The ABI version is `2`. Version 2 retains the version-1 one-shot entry point
and adds opaque, page-owned session handles for repeated inline source.

## API

```c
tsvm_status tsvm_execute_utf8(const unsigned char* source,
                              size_t source_len,
                              tsvm_result** out_result);
const unsigned char* tsvm_result_json(const tsvm_result* result,
                                      size_t* out_len);
void tsvm_result_free(tsvm_result* result);
tsvm_status tsvm_page_session_create(size_t cache_capacity,
                                     tsvm_page_session** out_session);
tsvm_status tsvm_page_session_execute_utf8(
    tsvm_page_session* session,
    const unsigned char* source,
    size_t source_len,
    tsvm_script_policy policy,
    tsvm_result** out_result);
tsvm_status tsvm_page_session_cache_stats(
    const tsvm_page_session* session,
    tsvm_cache_stats* out_stats);
void tsvm_page_session_free(tsvm_page_session* session);
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

## Page Sessions

`tsvm_page_session_create` allocates one opaque Rust-owned session containing a
bounded verified-module preparation cache. Capacity must be nonzero. A valid
`out_session` is reset to null before creation; null output pointers and zero
capacity return `TSVM_STATUS_INVALID_ARGUMENT`. Each successful session is
owned exclusively by its caller, must be used on one serialized sequence, and
is released exactly once with `tsvm_page_session_free`. Freeing a null session
is valid.

`tsvm_page_session_execute_utf8` accepts inline source only. It has the same
length-delimited UTF-8 rule as the one-shot entry point. It checks the supplied
policy on every call before the session can look up or prepare source:

```c
typedef enum tsvm_script_policy {
  TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT = 0,
  TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT = 1,
} tsvm_script_policy;
```

An unknown policy, null session, null output pointer, or non-empty null source
returns `TSVM_STATUS_INVALID_ARGUMENT` without a result. Invalid UTF-8,
compile/verify/runtime failures, and a blocked policy request return an owned
error result. A blocked request returns `TSVM_STATUS_RUNTIME_ERROR` and leaves
the cache counters unchanged.

`tsvm_page_session_cache_stats` copies observation data into caller-owned
storage; it never exposes a borrowed cache reference:

```c
typedef struct tsvm_cache_stats {
  size_t hits;
  size_t misses;
  size_t evictions;
  size_t entries;
} tsvm_cache_stats;
```

Identical allowed inline text produces a preparation miss first and a hit on
later calls until FIFO eviction. The cache retains verified preparation only:
every execution still creates a fresh TSVM runtime and heap and receives a
fresh empty `HostEnvironment`. The session stores no source buffer, result,
host capability, DOM, or browser network state.

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
auto created = tsvm::chromium::PageSession::Create(8);
if (created.status != TSVM_STATUS_OK) return;
auto session = std::move(created.session);

const auto result = session.ExecuteInline(
    "console.log(150);", TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT);
const auto cache = session.CacheStats();
```

`PageSession` is move-only. Its destructor releases the opaque Rust handle;
`ExecuteInline` copies its temporary JSON before freeing the temporary Rust
result; and `CacheStats` returns both the C status and copied counters. The
legacy `ExecuteSource` wrapper remains available for one-shot callers.

The wrapper owns no V8, Blink, DOM, IPC, filesystem, network capability, source
buffer, result bytes, runtime heap, or host environment. A future Blink script
hook must run normal browser script policy, CSP, origin, site-isolation, and
resource-loading checks before it supplies concrete inline source to this
bridge. The bridge belongs in a renderer process, never a privileged browser
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

Run the local linked smoke with:

```powershell
cmd /d /s /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 && cargo build -p tsvm-c-api && cl /nologo /std:c++20 /EHsc /I browser\chromium /I runtime\c-api\include browser\chromium\tsvm_renderer_bridge.cc browser\chromium\renderer_bridge_smoke.cc target\debug\tsvm_c_api.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib /Fe:target\debug\tsvm_renderer_bridge_smoke.exe && target\debug\tsvm_renderer_bridge_smoke.exe'
```

It proves a real C++ wrapper observes one preparation miss, one cache hit, and
a blocked request that leaves counters unchanged. This is a local native bridge
proof, not a Chromium build or browser performance benchmark.
