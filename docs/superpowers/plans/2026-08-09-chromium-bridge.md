# TSVM Chromium Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide a stable C ABI and dependency-light C++ adapter that invoke the verified TSVM pipeline without a JavaScript fallback.

**Architecture:** `runtime/c-api` owns UTF-8 validation, error mapping, deterministic result serialization, opaque result allocation, and the narrow raw-pointer adapter. `browser/chromium` owns a C++20 RAII wrapper that copies the ABI result before freeing it. Neither layer owns browser capabilities or V8 values.

**Tech Stack:** Rust 2021, standard C ABI, C++20, Cargo, GitHub Actions.

## Global Constraints

- Preserve `TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM`.
- Never introduce a TypeScript-to-JavaScript execution or serialization path.
- Keep unsafe Rust confined to `runtime/c-api`; the rest of the workspace keeps `unsafe_code = forbid`.
- Export ABI version 1 and retain opaque-result ownership entirely in Rust.
- Run formatting, Clippy, workspace tests, C++ syntax verification, and a real C ABI smoke test before committing.

---

### Task 1: Establish the ABI Contract With Failing Tests

**Files:**
- Create: `runtime/c-api/Cargo.toml`
- Create: `runtime/c-api/src/lib.rs`
- Create: `runtime/c-api/tests/c_api.rs`
- Create: `runtime/c-api/include/tsvm_c_api.h`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `tsvm_interpreter::execute_source(&str) -> Result<ExecutionOutput, ExecuteError>`.
- Produces: `Status`, `CApiResult`, `execute_utf8`, `tsvm_execute_utf8`, `tsvm_result_json`, `tsvm_result_free`, and `tsvm_abi_version`.

- [ ] **Step 1: Write the failing ABI tests**

```rust
#[test]
fn exported_abi_runs_the_initial_demo_without_generated_javascript() {
    let source = b"console.log(150);";
    let mut result = std::ptr::null_mut();
    let status = unsafe { tsvm_execute_utf8(source.as_ptr(), source.len(), &mut result) };
    assert_eq!(status, Status::Ok);
    let json = unsafe { result_json(result) };
    assert!(json.contains("\"generated_javascript\":false"));
    assert!(json.contains("\"value\":150"));
    unsafe { tsvm_result_free(result) };
}
```

Also add tests for invalid UTF-8, a semantic error, a null non-empty source,
and a null output pointer. Name the production change each test detects in a
comment before the assertion.

- [ ] **Step 2: Run the new test target to verify it fails**

Run: `cargo +stable-x86_64-pc-windows-gnu test -p tsvm-c-api --test c_api`

Expected: FAIL because package `tsvm-c-api` and the exported ABI do not exist.

- [ ] **Step 3: Add the package and public C header**

Create a `staticlib` and `cdylib` package depending only on `tsvm-interpreter`.
Define the exact status values in both Rust and `tsvm_c_api.h`; add C/C++ guards,
an export macro, `<stddef.h>`, and the opaque `tsvm_result` declaration. Add
`runtime/c-api` to workspace members. Configure the package's local lint policy
to permit only its FFI adapter's unsafe operations.

- [ ] **Step 4: Implement the safe core and thin FFI adapter**

```rust
pub fn execute_utf8(source: &[u8]) -> CApiResult {
    let source = match std::str::from_utf8(source) {
        Ok(source) => source,
        Err(error) => return CApiResult::error(Status::InvalidUtf8, error.to_string()),
    };
    match tsvm_interpreter::execute_source(source) {
        Ok(output) => CApiResult::success(render_output(&output)),
        Err(error) => CApiResult::error(Status::from_execute_error(&error), error.to_string()),
    }
}
```

Use `catch_unwind` around every C export. Validate pointer/length combinations
before creating slices, initialize a valid output pointer to null first, and
allocate results with `Box`. Serialize strings with a local JSON-escape helper;
tag all result values and include `generated_javascript:false`.

- [ ] **Step 5: Run the ABI tests to verify they pass**

Run: `cargo +stable-x86_64-pc-windows-gnu test -p tsvm-c-api --test c_api`

Expected: PASS for demo, UTF-8, compile failure, and invalid-argument cases.

- [ ] **Step 6: Refactor only after green**

Extract JSON escaping and value rendering helpers if they are duplicated, then
rerun the same command with no behavior changes.

### Task 2: Add the Renderer-Facing C++20 Adapter

**Files:**
- Create: `browser/chromium/tsvm_renderer_bridge.h`
- Create: `browser/chromium/tsvm_renderer_bridge.cc`
- Create: `browser/chromium/renderer_bridge_smoke.cc`

**Interfaces:**
- Consumes: ABI v1 declarations from `runtime/c-api/include/tsvm_c_api.h`.
- Produces: `tsvm::chromium::ExecutionResult ExecuteSource(std::string_view)`.

- [ ] **Step 1: Write the failing compile smoke program**

```cpp
#include "tsvm_renderer_bridge.h"

int main() {
  const auto result = tsvm::chromium::ExecuteSource("console.log(150);");
  return result.json.empty() ? 1 : 0;
}
```

- [ ] **Step 2: Run the syntax-only compile to verify it fails**

Run: `c++ -std=c++20 -fsyntax-only browser/chromium/renderer_bridge_smoke.cc -Ibrowser/chromium -Iruntime/c-api/include`

Expected: FAIL because `tsvm_renderer_bridge.h` does not exist.

- [ ] **Step 3: Implement the wrapper**

```cpp
namespace tsvm::chromium {
struct ExecutionResult {
  tsvm_status status;
  std::string json;
};

ExecutionResult ExecuteSource(std::string_view source);
}  // namespace tsvm::chromium
```

Call `tsvm_execute_utf8`, copy the byte payload returned by `tsvm_result_json`,
then free the result exactly once. Return an empty payload only when the ABI
cannot allocate a result. Do not include Chromium, Blink, V8, DOM, or IPC
headers.

- [ ] **Step 4: Re-run the syntax-only compile to verify it passes**

Run: `c++ -std=c++20 -fsyntax-only browser/chromium/tsvm_renderer_bridge.cc browser/chromium/renderer_bridge_smoke.cc -Ibrowser/chromium -Iruntime/c-api/include`

Expected: PASS.

### Task 3: Document and Automate the Boundary

**Files:**
- Create: `docs/c-api.md`
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/roadmap.md`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: ABI header, Rust export behavior, C++ adapter.
- Produces: user-facing integration instructions and a CI guard for C++ syntax.

- [ ] **Step 1: Write documentation assertions as a CI failure condition**

Add a CI documentation command that requires `docs/c-api.md` and searches it
for `tsvm_execute_utf8`, `tsvm_result_free`, and `generated_javascript`.

- [ ] **Step 2: Run the docs job command locally to verify it fails**

Run: `Test-Path docs/c-api.md`

Expected: `False` before the document is created.

- [ ] **Step 3: Write the API and architecture documentation**

Document the ABI version, status meanings, pointer rules, result ownership,
JSON envelope, Rust build artifacts, C++ wrapper usage, no-JavaScript invariant,
and Chromium checkout disk prerequisite. Add the bridge to README and
architecture, and change M10/M9 remaining work to note that the C ABI
foundation is complete while Blink integration remains blocked by the absent
checkout.

- [ ] **Step 4: Add and run CI-equivalent checks**

Add this Linux CI step after Rust tests:

```yaml
- name: C++ renderer bridge syntax
  run: >-
    c++ -std=c++20 -fsyntax-only
    browser/chromium/tsvm_renderer_bridge.cc
    browser/chromium/renderer_bridge_smoke.cc
    -Ibrowser/chromium -Iruntime/c-api/include
```

Run the matching local compiler command and verify the documentation search
finds all three required API terms.

### Task 4: Verify, Commit, and Sync the Milestone

**Files:**
- Modify: `docs/superpowers/plans/2026-08-09-chromium-bridge.md` (check tasks)

**Interfaces:**
- Consumes: completed bridge, documentation, and CI configuration.
- Produces: a verified, pushed bridge milestone.

- [ ] **Step 1: Run full verification**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-gnu test --workspace
c++ -std=c++20 -fsyntax-only browser/chromium/tsvm_renderer_bridge.cc browser/chromium/renderer_bridge_smoke.cc -Ibrowser/chromium -Iruntime/c-api/include
```

Expected: all commands exit 0.

- [ ] **Step 2: Inspect the final diff**

Run: `git diff --check` and `git status --short`.

Expected: no whitespace errors and only bridge milestone files staged for commit.

- [ ] **Step 3: Commit and push**

```powershell
git add Cargo.toml Cargo.lock runtime/c-api browser/chromium docs README.md .github/workflows/ci.yml
git commit -m "feat: add TSVM Chromium C ABI bridge"
git push origin main
```

Expected: one focused milestone commit is visible on `origin/main`.
