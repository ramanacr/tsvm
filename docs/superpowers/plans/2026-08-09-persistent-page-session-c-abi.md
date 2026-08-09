# Persistent Page-Session C ABI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose M16's bounded, policy-aware inline TypeScript page session through the versioned TSVM C ABI and a C++20 RAII adapter, without adding a JavaScript path or claiming Chromium integration.

**Architecture:** Keep the existing one-shot `tsvm_execute_utf8` ABI intact and add an opaque Rust-owned `tsvm_page_session` that contains one `PageScriptSession`. Each C ABI execution call validates raw inputs, maps the policy, makes a fresh empty `HostEnvironment`, and lets the script loader enforce policy before cache access. C++ owns the opaque session with a move-only RAII wrapper and copies temporary result bytes before releasing Rust-owned results.

**Tech Stack:** Rust 2024 workspace crates, `tsvm-script-loader`, `tsvm-interop`, C ABI (`extern "C"`), C++20, MSVC local smoke link, GitHub Actions Ubuntu syntax validation.

## Global Constraints

- Advance `tsvm_abi_version()` from `1` to `2`; retain every existing one-shot ABI signature and behavior.
- The C ABI accepts length-delimited UTF-8; a non-empty null source pointer is invalid and an empty source may use null.
- `tsvm_page_session_create` accepts only nonzero capacities and writes `*out_session = NULL` before fallible work.
- The C ABI must validate raw policy values as an integer before converting them to `ScriptPolicy`; unknown values return `TSVM_STATUS_INVALID_ARGUMENT`.
- Allowed calls execute direct TypeScript through `PageScriptSession`; no TypeScript-to-JavaScript path, JIT, disk cache, process cache, external-resource ABI, DOM binding, or fetch binding is added.
- Every call uses a fresh empty `HostEnvironment` and gets fresh TSVM runtime/heap state, even when the page-session preparation cache hits.
- The script policy is applied by `PageScriptSession` before cache lookup; blocked calls return an owned runtime-error result and do not mutate cache counters.
- Result JSON remains deterministic and records `"generated_javascript":false`; cache telemetry is exposed only through `tsvm_cache_stats`.
- Opaque session ownership is exclusive and callers must serialize use; null free is valid and use after free is invalid.
- C++ must store no source, result bytes, host capability, runtime heap, DOM state, or borrowed Rust byte view beyond its copy operation.
- Use `cargo +stable-x86_64-pc-windows-gnu` for Rust verification on this Windows workspace and use the configured Visual Studio 2026 environment for the MSVC C++ smoke link.
- Do not publish a new wall-clock benchmark result for M17. M16 remains the published cache-performance baseline because M17 is an ownership/API milestone.

---

## File Structure

- `runtime/c-api/Cargo.toml`: Add the direct script-loader dependency required by the opaque session implementation.
- `runtime/c-api/include/tsvm_c_api.h`: Define the opaque session, script policy, copied cache-stat value, and v2 exported C functions.
- `runtime/c-api/src/lib.rs`: Own `PageScriptSession`, validate the ABI contract, contain panics, execute inline source, and copy cache counters.
- `runtime/c-api/tests/c_api.rs`: Exercise the legacy ABI and every C session success/error/ownership contract through exported functions.
- `browser/chromium/tsvm_renderer_bridge.h`: Publish move-only `PageSession`, `PageSessionCreation`, and C++ cache-stat result types alongside `ExecuteSource`.
- `browser/chromium/tsvm_renderer_bridge.cc`: Implement C++ result copying and RAII lifecycle over the C ABI.
- `browser/chromium/renderer_bridge_smoke.cc`: Link and run the C++ wrapper against a real static library, proving cache reuse and policy-first blocking.
- `README.md`, `docs/c-api.md`, `docs/roadmap.md`, `docs/milestones.md`, `docs/adr/ADR-0003-rust-cpp-boundary.md`: Describe v2 ownership, exact limitations, and M17 roadmap state.
- `.github/workflows/ci.yml`: Keep the existing C++20 syntax check valid for the expanded public C++ interface; no new platform-only link job is required for this milestone.

## Task 1: Version-2 Rust C ABI Page Session

**Files:**
- Modify: `runtime/c-api/Cargo.toml`
- Modify: `runtime/c-api/include/tsvm_c_api.h`
- Modify: `runtime/c-api/src/lib.rs`
- Modify: `runtime/c-api/tests/c_api.rs`

**Interfaces:**
- Consumes: `tsvm_script_loader::PageScriptSession`, `tsvm_script_loader::ScriptPolicy`, `tsvm_interop::HostEnvironment`, and `tsvm_interpreter::{ExecuteError, ExecutionOutput}`.
- Produces: `TSVM_ABI_VERSION == 2`, opaque `TsvmPageSession`, `CacheStats`, `tsvm_page_session_create`, `tsvm_page_session_execute_utf8`, `tsvm_page_session_cache_stats`, and `tsvm_page_session_free`.

- [ ] **Step 1: Add failing exported-ABI integration tests**

Extend the test imports and add these tests to `runtime/c-api/tests/c_api.rs`. Keep the tests at the ABI level: they must call exported functions and release every non-null result/session.

```rust
use tsvm_c_api::{
    tsvm_page_session_cache_stats, tsvm_page_session_create,
    tsvm_page_session_execute_utf8, tsvm_page_session_free, CacheStats,
    TsvmPageSession, TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT,
    TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT,
};

#[test]
fn page_session_reuses_preparation_but_not_execution_state() {
    let mut session = ptr::null_mut();
    assert_eq!(unsafe { tsvm_page_session_create(1, &mut session) }, Status::Ok);

    for _ in 0..2 {
        let mut result = ptr::null_mut();
        let source = b"console.log(150);";
        assert_eq!(
            unsafe {
                tsvm_page_session_execute_utf8(
                    session, source.as_ptr(), source.len(),
                    TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT, &mut result,
                )
            },
            Status::Ok,
        );
        assert!(unsafe { result_json(result) }.contains("\"generated_javascript\":false"));
        unsafe { tsvm_result_free(result) };
    }

    let mut stats = CacheStats::default();
    assert_eq!(unsafe { tsvm_page_session_cache_stats(session, &mut stats) }, Status::Ok);
    assert_eq!((stats.hits, stats.misses, stats.evictions, stats.entries), (1, 1, 0, 1));
    unsafe { tsvm_page_session_free(session) };
}

#[test]
fn blocked_page_session_call_does_not_mutate_cache_stats() {
    let mut session = ptr::null_mut();
    assert_eq!(unsafe { tsvm_page_session_create(1, &mut session) }, Status::Ok);
    let source = b"console.log(150);";
    let mut result = ptr::null_mut();
    assert_eq!(
        unsafe {
            tsvm_page_session_execute_utf8(
                session, source.as_ptr(), source.len(),
                TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT, &mut result,
            )
        },
        Status::RuntimeError,
    );
    assert!(unsafe { result_json(result) }.contains("\"status\":\"runtime_error\""));
    unsafe { tsvm_result_free(result) };

    let mut stats = CacheStats::default();
    assert_eq!(unsafe { tsvm_page_session_cache_stats(session, &mut stats) }, Status::Ok);
    assert_eq!((stats.hits, stats.misses, stats.evictions, stats.entries), (0, 0, 0, 0));
    unsafe { tsvm_page_session_free(session) };
}
```

Add focused tests for zero capacity and null creation output; null session, null result output, non-empty null source, unknown raw policy integer, invalid UTF-8, compile error, and null stats input/output. Assert that result output stays null for invalid-argument cases, but invalid UTF-8, compile error, and policy blocking produce owned error results. Update the version assertion to `2`, and retain the existing legacy one-shot test unchanged.

- [ ] **Step 2: Run the new C ABI tests to verify they fail**

Run:

```powershell
$cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu test -p tsvm-c-api --test c_api'
& cmd.exe /d /s /c $cmd
```

Expected: compilation fails because the session symbols, C cache-stat type, and policy constants do not exist yet.

- [ ] **Step 3: Define the v2 C header and Rust ABI values**

In `runtime/c-api/include/tsvm_c_api.h`, include `<stddef.h>`, add the opaque `tsvm_page_session`, the exact `tsvm_script_policy` constants, `tsvm_cache_stats`, and these declarations after the legacy functions:

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

In `runtime/c-api/src/lib.rs`, set `TSVM_ABI_VERSION` to `2`, expose matching `#[repr(C)]` `CacheStats` with `Default`, retain `Status` as the return enum, and define the raw C numeric policy constants as `pub const ...: c_int = 0/1`. Receive the policy parameter as `c_int`, not a Rust enum, then validate it with:

```rust
fn script_policy_from_raw(policy: c_int) -> Result<ScriptPolicy, Status> {
    match policy {
        TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT => Ok(ScriptPolicy::default()),
        TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT => Ok(ScriptPolicy { allow_typescript: false }),
        _ => Err(Status::InvalidArgument),
    }
}
```

This preserves the C header's convenient enum while ensuring an unknown value arriving from C cannot become an invalid Rust enum discriminant.

- [ ] **Step 4: Implement opaque lifecycle, execution, panic containment, and copied stats**

Add direct `tsvm-script-loader` and `tsvm-interop` dependencies. Store the page-owned cache only in this Rust allocation:

```rust
pub struct TsvmPageSession {
    session: PageScriptSession,
}
```

Implement the functions with the existing `source_from_raw` and `write_result` helpers. Creation validates `out_session`, writes null first, calls `PageScriptSession::new(cache_capacity)`, converts a zero-capacity error to `Status::InvalidArgument`, allocates `Box<TsvmPageSession>`, and catches panics as `Status::InternalError`.

For execution, validate `session` and `out_result`, write `*out_result = null_mut()`, validate source and policy, decode UTF-8, then invoke:

```rust
let output = page.session.execute_inline_typescript(
    source,
    &HostEnvironment::new(),
    policy,
);
```

Map `ScriptLoaderError { source: Some(error), .. }` through the existing `ExecuteError` status mapping and a policy-only loader failure to `Status::RuntimeError`. Render success through the existing result envelope and render every error through an owned `TsvmResult`. Catch panics and return an owned internal-error result after the output pointer has been initialized.

For stats, validate both pointers before dereference, copy `hits`, `misses`, `evictions`, and `entries` from `page.session.cache_stats()` into `CacheStats`, and catch panics as `Status::InternalError`. Free accepts null and drops a live `Box<TsvmPageSession>` exactly once. Keep legacy ABI code and its ownership rules unchanged.

- [ ] **Step 5: Run focused and workspace Rust verification**

Run:

```powershell
$cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu fmt --all -- --config newline_style=Windows --check && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu clippy -p tsvm-c-api --all-targets -- -D warnings && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu test -p tsvm-c-api --test c_api && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu test -p tsvm-script-loader'
& cmd.exe /d /s /c $cmd
```

Expected: all focused checks pass, including a legacy one-shot execution, a cache miss then hit, no counter mutation for blocked policy, and each pointer/UTF-8 contract.

- [ ] **Step 6: Commit and publish the ABI core milestone**

```powershell
git add runtime/c-api/Cargo.toml runtime/c-api/include/tsvm_c_api.h runtime/c-api/src/lib.rs runtime/c-api/tests/c_api.rs
git commit -m "feat: expose persistent page sessions through C ABI"
git push origin HEAD:main
```

Expected: the focused, independently testable ABI milestone is committed and synchronized before C++ wrapper work begins.

## Task 2: Move-Only C++20 Page Session Adapter And Executable Proof

**Files:**
- Modify: `browser/chromium/tsvm_renderer_bridge.h`
- Modify: `browser/chromium/tsvm_renderer_bridge.cc`
- Modify: `browser/chromium/renderer_bridge_smoke.cc`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: all Task 1 C functions and types, especially `tsvm_page_session*`, `tsvm_script_policy`, `tsvm_cache_stats`, and `tsvm_result`.
- Produces: `tsvm::chromium::PageSession`, `PageSessionCreation`, `PageSessionCacheStats`, and a smoke executable that runs through the real static Rust library.

- [ ] **Step 1: Expand the smoke test first**

Replace the one-shot-only check in `browser/chromium/renderer_bridge_smoke.cc` with a session lifecycle proof:

```cpp
int main() {
  const auto created = tsvm::chromium::PageSession::Create(1);
  if (created.status != TSVM_STATUS_OK || !created.session.is_valid()) return 1;

  auto session = std::move(created.session);
  const auto first = session.ExecuteInline(
      "console.log(150);", TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT);
  const auto second = session.ExecuteInline(
      "console.log(150);", TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT);
  if (first.json.empty() || second.json.empty()) return 1;

  const auto stats = session.CacheStats();
  if (stats.status != TSVM_STATUS_OK || stats.hits != 1 || stats.misses != 1 ||
      stats.evictions != 0 || stats.entries != 1) return 1;

  const auto blocked = session.ExecuteInline(
      "console.log(151);", TSVM_SCRIPT_POLICY_BLOCK_TYPESCRIPT);
  if (blocked.status != TSVM_STATUS_RUNTIME_ERROR || blocked.json.empty()) return 1;
  const auto after_block = session.CacheStats();
  return after_block.hits == 1 && after_block.misses == 1 &&
                 after_block.evictions == 0 && after_block.entries == 1
             ? 0
             : 1;
}
```

Include `<utility>` for the explicit move. The source exists only during `ExecuteInline`; the wrapper must not retain it.

- [ ] **Step 2: Run the C++ syntax check to verify it fails**

Run:

```powershell
$cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && cl /nologo /std:c++20 /Zs /I browser\chromium /I runtime\c-api\include browser\chromium\tsvm_renderer_bridge.cc browser\chromium\renderer_bridge_smoke.cc'
& cmd.exe /d /s /c $cmd
```

Expected: compilation fails because `PageSession`, creation, inline execution, and cache stats are not in the C++ adapter yet.

- [ ] **Step 3: Define the C++ ownership API and implement it**

In the header, retain `ExecutionResult` and `ExecuteSource`. Add a copied stats value and a creation return value:

```cpp
struct PageSessionCacheStats {
  tsvm_status status = TSVM_STATUS_INTERNAL_ERROR;
  size_t hits = 0;
  size_t misses = 0;
  size_t evictions = 0;
  size_t entries = 0;
};

class PageSession {
 public:
  static PageSessionCreation Create(size_t cache_capacity);
  PageSession(PageSession&& other) noexcept;
  PageSession& operator=(PageSession&& other) noexcept;
  PageSession(const PageSession&) = delete;
  PageSession& operator=(const PageSession&) = delete;
  ~PageSession();

  [[nodiscard]] bool is_valid() const;
  ExecutionResult ExecuteInline(std::string_view source,
                                tsvm_script_policy policy);
  PageSessionCacheStats CacheStats() const;

 private:
  explicit PageSession(tsvm_page_session* session);
  tsvm_page_session* session_ = nullptr;
};

struct PageSessionCreation {
  tsvm_status status;
  PageSession session;
};
```

Order the declarations so `PageSessionCreation` is forward-declared before the class and defined after it. In the implementation, `Create` initializes `tsvm_page_session* raw = nullptr`, invokes `tsvm_page_session_create`, and returns the status plus the wrapper. The destructor and move assignment release only a non-null owned handle, using `std::exchange` to leave moved-from instances null. `ExecuteInline` follows the existing `ExecuteSource` pattern, calls `tsvm_page_session_execute_utf8`, copies JSON with `CopyJson`, and lets `ResultHandle` release the temporary result. `CacheStats` calls the C stats API and copies values only when it returns `TSVM_STATUS_OK`.

Do not add exceptions, source retention, or a fake success value for an invalid page-session handle. For an invalid wrapper, `ExecuteInline` returns `{TSVM_STATUS_INVALID_ARGUMENT, {}}` and `CacheStats` returns `{TSVM_STATUS_INVALID_ARGUMENT, 0, 0, 0, 0}` without calling Rust.

- [ ] **Step 4: Make CI validate the expanded C++ interface**

Keep the existing Ubuntu `c++ -std=c++20 -fsyntax-only` step and update only its display name to `C++ renderer bridge syntax and page-session smoke`. The command already compiles both bridge sources against the public header, so it validates the new declarations without pretending to link a Linux Rust static library. The Windows link-and-run command remains documented and executed locally as the authoritative executable proof.

- [ ] **Step 5: Build, link, and execute the C++ smoke against the Rust static library**

Run:

```powershell
$cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" build -p tsvm-c-api && cl /nologo /std:c++20 /EHsc /I browser\chromium /I runtime\c-api\include browser\chromium\tsvm_renderer_bridge.cc browser\chromium\renderer_bridge_smoke.cc target\debug\tsvm_c_api.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib /Fe:target\debug\tsvm_renderer_bridge_smoke.exe && target\debug\tsvm_renderer_bridge_smoke.exe'
& cmd.exe /d /s /c $cmd
```

Expected: exit code `0`; it proves the real C++ wrapper observes one cache miss, one hit, then a blocked request that leaves those counters unchanged. Remove only the two root-level `tsvm_renderer_bridge.obj` and `renderer_bridge_smoke.obj` build by-products after verifying their resolved paths remain inside the repository.

- [ ] **Step 6: Commit and publish the C++ bridge milestone**

```powershell
git add browser/chromium/tsvm_renderer_bridge.h browser/chromium/tsvm_renderer_bridge.cc browser/chromium/renderer_bridge_smoke.cc .github/workflows/ci.yml
git commit -m "feat: add C++ page session bridge"
git push origin HEAD:main
```

Expected: the C++20 ownership adapter and executable proof are committed and synchronized as a separate milestone.

## Task 3: Documentation, Roadmap, And Full Release-Quality Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/c-api.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/milestones.md`
- Modify: `docs/adr/ADR-0003-rust-cpp-boundary.md`

**Interfaces:**
- Consumes: Task 1 v2 ABI and Task 2 C++ API.
- Produces: Accurate user-facing ownership/usage guidance and M17 roadmap evidence with no unsupported browser or performance claims.

- [ ] **Step 1: Update public C ABI documentation with a usable v2 lifecycle**

In `docs/c-api.md`, retain the legacy one-shot example and add a C++ example using the actual wrapper API:

```cpp
const auto created = tsvm::chromium::PageSession::Create(8);
if (created.status != TSVM_STATUS_OK) return;
auto session = std::move(created.session);

const auto result = session.ExecuteInline(
    "console.log(150);", TSVM_SCRIPT_POLICY_ALLOW_TYPESCRIPT);
const auto cache = session.CacheStats();
```

Document `PageSession` as move-only, thread/sequence-exclusive, and responsible for one page-owned preparation cache. State exactly that repeated equal inline source can reuse verified preparation but every execution receives a fresh TSVM runtime and empty host. List all pointer, UTF-8, policy, error-result, null-free, and result/session release rules. Show the actual MSVC build/link/run smoke command and say it is an integration proof for the narrow bridge, not a Chromium build.

- [ ] **Step 2: Update README, roadmap, milestones, and ADR status**

Update `README.md` current status and Native C ABI section to say M17 is implemented: the native boundary now includes opaque page sessions, direct inline source only, policy-first cache access, copied stats, fresh per-execution runtime/host, and C++20 RAII. Link to `docs/c-api.md` and reiterate that real Chromium/Blink dispatch remains future work.

Add an M17 row to `docs/roadmap.md` with status `Done`. State it provides ABI v2 persistent page-session ownership, policy-aware direct inline execution, copied stats, and C++ RAII proof; defer actual Blink dispatch, resource loading, CSP/site-isolation integration, origin cache identity/partitioning, DOM/fetch, and browser lifecycle invalidation. Update the summary and near-term priorities to place M17 before a future Chromium checkout dispatch hook.

Append an M17 section to `docs/milestones.md` with acceptance evidence: legacy ABI remains source-compatible, v2 creates/destroys bounded opaque sessions, policy is enforced before cache lookup, stats are copied values, fresh runtime/host is retained per execution, C++ smoke validates miss/hit/blocked behavior, and no M17 benchmark is claimed. List the same browser integration work as deferred.

Update `docs/adr/ADR-0003-rust-cpp-boundary.md` consequences with the v2 opaque session ownership rules: Rust owns allocation/panic containment, C++ uses move-only RAII, C++ copies borrowed result data before release, and future browser policy/resource checks stay outside this narrow source-only ABI.

- [ ] **Step 3: Run documentation and full runtime verification**

Run:

```powershell
$cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu fmt --all -- --config newline_style=Windows --check && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu test --workspace && "C:\Users\Raman\.cargo\bin\cargo.exe" +stable-x86_64-pc-windows-gnu run -p tsvm-lexer --bin lexer_corpus_runner -- tests/fixtures/lexer'
& cmd.exe /d /s /c $cmd
```

Then run the Task 2 MSVC link-and-execute command again. Expected: format, lint, all workspace tests, lexer corpus, and the real C++ smoke pass. Do not rerun or alter the M16 release benchmark: M17 changes the embedding API rather than the benchmarked standalone workload.

- [ ] **Step 4: Commit and publish the documentation milestone**

```powershell
git add README.md docs/c-api.md docs/roadmap.md docs/milestones.md docs/adr/ADR-0003-rust-cpp-boundary.md
git commit -m "docs: document persistent page session ABI"
git push origin HEAD:main
```

Expected: user-facing documentation, roadmap state, and the M17 verification evidence are synchronized to GitHub. Confirm the resulting GitHub Actions run succeeds and report its URL with the milestone summary.

