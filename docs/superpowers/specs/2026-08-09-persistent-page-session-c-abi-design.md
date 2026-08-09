# Persistent Page-Session C ABI Design

## Purpose

M17 carries M16's standalone page-session preparation cache across the existing
Rust/C++ renderer boundary. A C++ renderer-facing owner will be able to create
one bounded TSVM session, execute repeated inline TypeScript against it, apply
the current script policy for every request, and observe cache statistics.

This is an integration foundation for a future Blink `text/typescript` dispatch
hook. It does not claim that a Chromium renderer, Blink dispatch, browser DOM,
or browser network stack exists in this repository.

## Scope

M17 delivers:

1. A version-2 additive C ABI for opaque persistent page sessions.
2. Policy-aware inline UTF-8 execution through `PageScriptSession`.
3. Explicit C ABI cache-stat observation and Rust/C++ ownership rules.
4. A move-only C++20 RAII wrapper and an executable smoke test that proves
   preparation reuse through the real ABI.
5. Documentation and roadmap evidence that precisely describes the boundary.

## Non-Goals

M17 does not add a Chromium checkout, Blink source change, V8 integration,
generated JavaScript, JIT, process/global cache, disk cache, browser network
cache, external module/resource ABI, DOM or fetch host binding, event loop,
timer, promise, async fetch, page navigation, or browser cache
invalidation/origin partitioning policy.

The ABI's page session handles inline source only. A real browser integration
must resolve external script resources and enforce browser policy before it
supplies concrete source to this narrow renderer-side boundary.

## ABI Design

`runtime/c-api/include/tsvm_c_api.h` advances `tsvm_abi_version()` from `1` to
`2`. Existing `tsvm_execute_utf8`, `tsvm_result_json`, and `tsvm_result_free`
retain their behavior and signatures.

The header introduces opaque ownership and value-only observation types:

```c
typedef struct tsvm_page_session tsvm_page_session;

typedef enum tsvm_script_policy {
  TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT = 0,
  TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT = 1,
} tsvm_script_policy;

typedef struct tsvm_cache_stats {
  size_t hits;
  size_t misses;
  size_t evictions;
  size_t entries;
} tsvm_cache_stats;
```

It adds these exported functions:

```c
TSVM_API tsvm_status tsvm_page_session_create(
    size_t cache_capacity,
    tsvm_page_session** out_session);

TSVM_API tsvm_status tsvm_page_session_execute_utf8(
    tsvm_page_session* session,
    const unsigned char* source,
    size_t source_len,
    tsvm_script_policy policy,
    tsvm_result** out_result);

TSVM_API tsvm_status tsvm_page_session_cache_stats(
    const tsvm_page_session* session,
    tsvm_cache_stats* out_stats);

TSVM_API void tsvm_page_session_free(tsvm_page_session* session);
```

All pointer/input rules are length-delimited and explicit. Creation rejects a
null output pointer and a zero capacity with `TSVM_STATUS_INVALID_ARGUMENT`.
Execution rejects a null session, invalid result output pointer, non-empty null
source pointer, or unknown policy value with that status. A malformed UTF-8
buffer returns `TSVM_STATUS_INVALID_UTF8` and an owned error result. A blocked
policy request returns `TSVM_STATUS_RUNTIME_ERROR` with an owned error result,
but does not perform cache lookup or source preparation.

`tsvm_page_session_free` accepts null. The caller owns the page-session handle
exclusively, may not use it after free, and must serialize calls for a handle.
M17 does not claim a session is safe to share across renderer sequences or
threads. `tsvm_result` ownership remains unchanged: every non-null result is
released exactly once with `tsvm_result_free`.

## Rust Implementation

`tsvm-c-api` adds a direct dependency on `tsvm-script-loader` and wraps:

```rust
struct TsvmPageSession {
    session: PageScriptSession,
}
```

Execution decodes UTF-8 using the same raw-buffer validation helper as the
one-shot ABI. It maps the C policy enum to `ScriptPolicy`, then calls
`PageScriptSession::execute_inline_typescript(source, &HostEnvironment::new(),
policy)`. `PageScriptSession` performs the authoritative policy check before
cache lookup. Its `ScriptLoaderError` maps to the existing status taxonomy:
embedded `ExecuteError` values map as today; a policy error without an embedded
runtime error maps to `RuntimeError`.

Every exported function validates pointers before dereference and runs its body
inside `catch_unwind`. Panics produce `InternalError`; output pointers are
written as null before fallible work. Cache stats are copied into the C value
struct, never exposed by reference. Success result JSON keeps
`"generated_javascript":false`; cache observation stays in the dedicated
stats API so consumers do not parse telemetry from result payloads.

## C++ Renderer Wrapper

`browser/chromium/tsvm_renderer_bridge.h/.cc` adds a move-only `PageSession`
RAII type. `PageSession::Create(capacity)` calls `tsvm_page_session_create` and
returns its status plus an owned wrapper. Its destructor calls
`tsvm_page_session_free`. `ExecuteInline(source, policy)` delegates to
`tsvm_page_session_execute_utf8`, copies the JSON before releasing its temporary
result handle, and returns the existing `ExecutionResult`. `CacheStats()` reads
the C value struct and reports its status rather than fabricating a default.

The existing one-shot `ExecuteSource` stays available and source-compatible.
The wrapper stores no source, result bytes, host capabilities, runtime heap,
or DOM state. It models renderer-owned cache lifetime only.

## Tests And Evidence

Rust C ABI tests prove:

- version `2` and legacy one-shot execution remain valid;
- creation rejects zero capacity and null output pointers;
- two allowed executions of identical source return direct TSVM output and
  produce one miss, one hit, and one cache entry;
- a blocked request leaves all cache counters unchanged;
- changed source, invalid UTF-8, compile errors, and invalid raw pointers keep
  the existing status/result ownership contract;
- stats rejects null input/output pointers without dereferencing them.

The C++20 smoke executable creates a one-entry session, executes
`console.log(150);` twice, verifies non-empty copied JSON and the expected
`{ hits: 1, misses: 1, evictions: 0, entries: 1 }` observation, then relies on
RAII destruction. It also verifies policy blocking does not change stats.

M17 retains workspace format/lint/test, lexer corpus, security hardening, C ABI
tests, and C++20 smoke verification. It adds no wall-clock performance claim:
M16 remains the published cache performance baseline because the M17 bridge is
an API/ownership foundation, not a browser-engine benchmark.

## Security Rules

- The only insertion path remains `PreparedModule::from_source` through
  `PageScriptSession` and `PreparedModuleCache`.
- Policy is applied on every ABI execution call before cache access.
- Cache identity remains exact source text and lifetime remains one opaque page
  session; M17 introduces no shared process or origin crossing cache.
- No host capability crosses or persists through the session API; M17 uses a
  fresh empty `HostEnvironment` per execution.
- Rust owns opaque allocation/destruction and catches panics before C++; C++
  copies borrowed result bytes before Rust frees their owner.
- No TypeScript-to-JavaScript path is added.

## Definition Of Done

M17 is complete when the versioned C ABI exposes and safely destroys bounded
page sessions, executes direct TypeScript with policy-first cache behavior,
reports copied cache stats, and preserves the legacy one-shot ABI. The C++ RAII
wrapper and smoke executable prove repeated direct execution plus cache
observation. User-facing docs and roadmap distinguish this boundary from real
Chromium/Blink integration, and all Rust/C++/security verification passes.

## Follow-On Work

M18 can connect `PageSession` to a real Chromium/Blink `text/typescript`
dispatch hook after normal browser script policy, resource loading, CSP, site
isolation, and origin checks. Browser-owned cache partitioning, invalidation,
network caching, DOM/fetch host bindings, and renderer sandbox verification
remain separate browser security design work.
