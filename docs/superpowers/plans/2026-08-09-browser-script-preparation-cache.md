# Browser Script Preparation Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cache verified TypeScript preparation inside a bounded page session so repeated browser-style scripts avoid recompilation while every run still receives fresh TSVM runtime state.

**Architecture:** `tsvm-interpreter` will own a bounded FIFO `PreparedModuleCache`, keyed solely by exact source text and populated only through `PreparedModule::from_source`. `tsvm-script-loader` will own that cache in `PageScriptSession`, applying `ScriptPolicy` before each lookup and caching resolved module-graph text only after normal resource resolution. The benchmark suite will use this public page-session path and publish the resulting cold-versus-cached evidence.

**Tech Stack:** Rust 1.82, Cargo workspace, Rust standard collections and timing, existing `tsvm-interpreter`, `tsvm-modules`, `tsvm-script-loader`, `tsvm-interop`, and `tsvm-web-bindings` crates.

## Global Constraints

- Preserve `TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM` for every cache insertion and execution.
- Add no JavaScript output or fallback, JIT, executable-code generation, global/process cache, disk cache, browser network cache, Chromium source change, timer, event loop, promise, or asynchronous fetch feature.
- The cache key is the exact resolved TypeScript source text; do not infer identity from URLs, origins, timestamps, or hashes.
- A cache entry is a private immutable `PreparedModule` produced only by `PreparedModule::from_source`; compile or verification failures are never inserted.
- Cache capacity zero is rejected. Eviction is deterministic FIFO: a hit never reorders entries.
- `PreparedModuleCacheStats` reports lookup `hits`, `misses`, `evictions`, and current `entries`; a failed source preparation still counts as one miss.
- Script policy must run before every page-session cache lookup and execution. A blocked policy request makes no cache lookup.
- The cache owns no host, heap, console, DOM, fetch state, or runtime values. Every module execution receives its caller-supplied `HostEnvironment` and creates fresh interpreter/heap state.
- Existing one-shot script-loader functions remain source-compatible by using a short-lived page session.
- Benchmarks run one unmeasured warm-up and five measured samples, validate every iteration, report medians in microseconds, and do not use a timing threshold in CI.
- Publish actual M16 release measurements with command, OS, CPU, Rust version, profile, sample methodology, raw CSV rows, and cache counter interpretation.
- Preserve existing verifier, script-policy, same-origin, C ABI, C++20 syntax, corpus, no-generated-JavaScript, workspace lint, and workspace test gates.
- Commit and push each completed milestone using Git author `ramanacr`.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `runtime/interpreter/src/lib.rs` | Define the cache error, lookup status, stats, immutable lookup borrow, and bounded FIFO verified-module cache. |
| `runtime/interpreter/tests/interpreter.rs` | Prove cache lookup accounting, failure handling, FIFO eviction, and execution through a cached borrow. |
| `browser/script-loader/Cargo.toml` | Add the workspace module-graph crate so the loader can cache resolved bundled source after resource resolution. |
| `browser/script-loader/src/lib.rs` | Add `PageScriptSession`; route inline and resolved external scripts through policy-first cached execution; keep compatibility wrappers. |
| `browser/script-loader/tests/script_loader.rs` | Prove session policy ordering, fresh state, fresh host capabilities, source changes, and external module source caching. |
| `tools/benchmarks/Cargo.toml` | Add the script-loader crate as the benchmark's public page-session dependency. |
| `tools/benchmarks/src/lib.rs` | Add `cached-entry`, cache-counter CSV columns, the cached page-startup scenario, and deterministic result validation. |
| `tools/benchmarks/tests/benchmarks.rs` | Assert scenario shape, cached counters, console counts, and expanded output contract. |
| `tools/demo/src/lib.rs`, `tools/demo/tests/demo.rs` | Keep the demonstration output aligned with the expanded benchmark CSV contract. |
| `docs/performance.md`, `docs/benchmark-results.md` | Explain the preparation-cache boundary, measurement method, counter semantics, and publish the M16 result set. |
| `docs/roadmap.md`, `docs/milestones.md`, `README.md` | Mark M16 accurately, document its standalone limits, and show users how to use the page session and benchmark. |

## Task 1: Verified Prepared-Module Cache

**Files:**
- Modify: `runtime/interpreter/src/lib.rs:3-117`
- Modify: `runtime/interpreter/tests/interpreter.rs:1-213`

**Interfaces:**
- Consumes: `PreparedModule::from_source(source: &str) -> Result<PreparedModule, ExecuteError>` and `PreparedModule::execute_with_host(&self, host: &HostEnvironment) -> Result<ExecutionOutput, ExecuteError>`.
- Produces: `PreparedModuleCache::new(capacity: usize) -> Result<PreparedModuleCache, PreparedModuleCacheError>`.
- Produces: `PreparedModuleCache::get_or_prepare(&mut self, source: &str) -> Result<CacheLookup<'_>, ExecuteError>` and `PreparedModuleCache::stats(&self) -> PreparedModuleCacheStats`.
- Produces: `CacheLookup::status(&self) -> CacheLookupStatus`, `CacheLookup::module(&self) -> &PreparedModule`, `CacheLookupStatus::{Hit, Miss}`, and public value-only stats/error types.

- [ ] **Step 1: Write failing cache tests**

  Extend the interpreter import and add these tests to `runtime/interpreter/tests/interpreter.rs`.

  ```rust
  use tsvm_interpreter::{
      CacheLookupStatus, ExecuteError, PreparedModuleCache, PreparedModuleCacheError, Value,
  };

  #[test]
  fn prepared_module_cache_reports_miss_then_hit_and_executes_cached_module() {
      let mut cache = PreparedModuleCache::new(2).expect("capacity should be valid");
      let first = cache
          .get_or_prepare("console.log(40 + 2);")
          .expect("first source should prepare");
      assert_eq!(first.status(), CacheLookupStatus::Miss);
      assert_eq!(first.module().execute().unwrap().console, vec![Value::Number(42.0)]);

      let second = cache
          .get_or_prepare("console.log(40 + 2);")
          .expect("cached source should execute");
      assert_eq!(second.status(), CacheLookupStatus::Hit);
      assert_eq!(cache.stats().hits, 1);
      assert_eq!(cache.stats().misses, 1);
      assert_eq!(cache.stats().evictions, 0);
      assert_eq!(cache.stats().entries, 1);
  }

  #[test]
  fn prepared_module_cache_rejects_zero_capacity() {
      assert!(matches!(
          PreparedModuleCache::new(0),
          Err(PreparedModuleCacheError::ZeroCapacity)
      ));
  }

  #[test]
  fn prepared_module_cache_evicts_oldest_inserted_source_without_reordering_hits() {
      let mut cache = PreparedModuleCache::new(2).unwrap();
      cache.get_or_prepare("console.log(1);").unwrap();
      cache.get_or_prepare("console.log(2);").unwrap();
      assert_eq!(cache.get_or_prepare("console.log(1);").unwrap().status(), CacheLookupStatus::Hit);
      cache.get_or_prepare("console.log(3);").unwrap();

      assert_eq!(cache.get_or_prepare("console.log(1);").unwrap().status(), CacheLookupStatus::Miss);
      assert_eq!(cache.stats().evictions, 2);
  }

  #[test]
  fn prepared_module_cache_does_not_insert_invalid_source() {
      let mut cache = PreparedModuleCache::new(1).unwrap();
      for _ in 0..2 {
          assert!(matches!(
              cache.get_or_prepare("const answer: number = \"bad\";"),
              Err(ExecuteError::Compile(_))
          ));
      }
      assert_eq!(cache.stats().misses, 2);
      assert_eq!(cache.stats().entries, 0);
  }
  ```

- [ ] **Step 2: Run the cache tests to verify they fail**

  Run:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-interpreter --test interpreter'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: compilation fails because the cache types and methods do not yet exist.

- [ ] **Step 3: Implement the bounded FIFO cache**

  In `runtime/interpreter/src/lib.rs`, import `VecDeque` alongside `BTreeMap` and add the following public contract after `PreparedModule`.

  ```rust
  #[derive(Debug, Copy, Clone, PartialEq, Eq)]
  pub enum CacheLookupStatus { Hit, Miss }

  #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
  pub struct PreparedModuleCacheStats {
      pub hits: usize,
      pub misses: usize,
      pub evictions: usize,
      pub entries: usize,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum PreparedModuleCacheError { ZeroCapacity }

  pub struct CacheLookup<'cache> {
      status: CacheLookupStatus,
      module: &'cache PreparedModule,
  }

  pub struct PreparedModuleCache {
      capacity: usize,
      modules: BTreeMap<String, PreparedModule>,
      insertion_order: VecDeque<String>,
      hits: usize,
      misses: usize,
      evictions: usize,
  }
  ```

  Implement `new`, `get_or_prepare`, and `stats`. On a hit, increment `hits` then borrow the map entry. On a miss, increment `misses`, call `PreparedModule::from_source(source)?`, evict `insertion_order.pop_front()` and its map entry when `modules.len() == capacity`, then insert `source.to_owned()` and append the exact key to the queue. Only call `stats` after the entry count is derived from `modules.len()`. `CacheLookup` exposes read-only `status` and `module` accessors; it never exposes bytecode or a mutable cache borrow.

- [ ] **Step 4: Run focused cache verification**

  Run:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-interpreter --test interpreter && "C:\Users\Raman\.cargo\bin\cargo.exe" clippy -p tsvm-interpreter --all-targets -- -D warnings'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: all interpreter integration tests and crate-local Clippy pass.

- [ ] **Step 5: Format and commit the verified cache milestone**

  Run `cargo fmt -p tsvm-interpreter -- --config newline_style=Windows`, then:

  ```powershell
  git add runtime/interpreter/src/lib.rs runtime/interpreter/tests/interpreter.rs
  git commit -m "feat: cache prepared verified modules"
  git push origin HEAD
  ```

  Expected: one focused commit authored by `ramanacr` is present on the remote branch.

## Task 2: Page-Owned Script Session

**Files:**
- Modify: `browser/script-loader/Cargo.toml:9-11`
- Modify: `browser/script-loader/src/lib.rs:3-146`
- Modify: `browser/script-loader/tests/script_loader.rs:1-86`

**Interfaces:**
- Consumes: Task 1 cache types and `tsvm_modules::bundle_module_graph(entry, sources)`.
- Produces: `PageScriptSession::new(cache_capacity: usize) -> Result<PageScriptSession, PreparedModuleCacheError>`.
- Produces: `PageScriptSession::cache_stats(&self) -> PreparedModuleCacheStats` and `PageScriptSession::execute_inline_typescript(&mut self, source: &str, host: &HostEnvironment, policy: ScriptPolicy) -> Result<ExecutionOutput, ScriptLoaderError>`.
- Produces: `PageScriptSession::execute_typescript_scripts_with_policy(&mut self, document_url: &str, html: &str, resources: &BTreeMap<String, String>, host: &HostEnvironment, policy: ScriptPolicy) -> Result<BrowserExecution, ScriptLoaderError>`.
- Produces: source-compatible free loader functions that instantiate `PageScriptSession::new(1).expect("one-shot session capacity is nonzero")` and delegate.

- [ ] **Step 1: Write failing page-session tests**

  Add `PageScriptSession` to the script-loader test import and add these focused cases.

  ```rust
  #[test]
  fn session_policy_blocks_before_cache_lookup() {
      let mut session = PageScriptSession::new(1).unwrap();
      let error = session
          .execute_inline_typescript(
              "console.log(42);",
              &HostEnvironment::new(),
              ScriptPolicy { allow_typescript: false },
          )
          .expect_err("policy should reject before preparation");
      assert!(error.message.contains("blocked"));
      assert_eq!(session.cache_stats().misses, 0);
  }

  #[test]
  fn session_reuses_preparation_with_fresh_runtime_and_host() {
      fn first_host(_args: &[tsvm_interop::InteropValue]) -> Result<tsvm_interop::InteropValue, tsvm_interop::InteropError> { Ok(tsvm_interop::InteropValue::Number(41.0)) }
      fn second_host(_args: &[tsvm_interop::InteropValue]) -> Result<tsvm_interop::InteropValue, tsvm_interop::InteropError> { Ok(tsvm_interop::InteropValue::Number(42.0)) }

      let source = "function pageValue(): number { return 0; } const state = { count: 1 }; state.count += 1; console.log(pageValue() + state.count);";
      let mut session = PageScriptSession::new(2).unwrap();
      let first = session.execute_inline_typescript(source, &HostEnvironment::new().with_function("pageValue", first_host), ScriptPolicy::default()).unwrap();
      let second = session.execute_inline_typescript(source, &HostEnvironment::new().with_function("pageValue", second_host), ScriptPolicy::default()).unwrap();

      assert_eq!(first.console, vec![Value::Number(43.0)]);
      assert_eq!(second.console, vec![Value::Number(44.0)]);
      assert_eq!(session.cache_stats().hits, 1);
      assert_eq!(session.cache_stats().misses, 1);
  }

  #[test]
  fn session_source_change_is_a_miss_and_resolved_module_source_is_cached() {
      let mut session = PageScriptSession::new(3).unwrap();
      session.execute_inline_typescript("console.log(40 + 2);", &HostEnvironment::new(), ScriptPolicy::default()).unwrap();
      session.execute_inline_typescript("console.log(40 + 3);", &HostEnvironment::new(), ScriptPolicy::default()).unwrap();
      let resources = BTreeMap::from([("/app.ts".into(), "console.log(42);".into())]);
      let html = r#"<script type=\"text/typescript\" src=\"/app.ts\"></script>"#;
      session.execute_typescript_scripts_with_policy("/index.html", html, &resources, &HostEnvironment::new(), ScriptPolicy::default()).unwrap();
      session.execute_typescript_scripts_with_policy("/index.html", html, &resources, &HostEnvironment::new(), ScriptPolicy::default()).unwrap();
      assert_eq!(session.cache_stats().misses, 3);
      assert_eq!(session.cache_stats().hits, 1);
  }
  ```

- [ ] **Step 2: Run the script-loader tests to verify they fail**

  Run:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-script-loader --test script_loader'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: compilation fails because `PageScriptSession` does not exist.

- [ ] **Step 3: Implement policy-first cached script execution**

  Add `tsvm-modules = { path = "../../runtime/modules" }` to `browser/script-loader/Cargo.toml`. Replace direct `execute_source_with_host` / `execute_module_graph` calls with a `PageScriptSession` that owns `PreparedModuleCache`.

  `execute_inline_typescript` must return the existing policy error before calling `get_or_prepare`. For a permitted source, map the preparation error to `ScriptLoaderError { message: "failed to execute inline TypeScript script".into(), source: Some(err) }`, then call the returned module's `execute_with_host(host)`.

  For an external script, preserve the current URL resolution and missing-resource errors. After resource existence is confirmed, call `bundle_module_graph(&specifier, resources)`, map module diagnostics to `ExecuteError::Module`, then cache `graph.bundled_source` through the same policy-first execution helper. Keep the external error message `failed to execute TypeScript script `{specifier}``; use the supplied host for execution. Append console output and `ScriptExecution` metadata exactly as today, and keep `generated_javascript: false`.

- [ ] **Step 4: Preserve the compatibility entry points**

  Make the three existing free functions construct a one-entry session and delegate to `PageScriptSession::execute_typescript_scripts_with_policy`. This preserves public signatures and one-shot behavior while ensuring all source execution shares the cache's verified preparation contract.

- [ ] **Step 5: Run focused loader verification**

  Run:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-script-loader --test script_loader && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-security-hardening && "C:\Users\Raman\.cargo\bin\cargo.exe" clippy -p tsvm-script-loader --all-targets -- -D warnings'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: new session tests, existing loader behavior, and security policy/no-JavaScript regressions pass.

- [ ] **Step 6: Format and commit the page-session milestone**

  Run `cargo fmt -p tsvm-script-loader -- --config newline_style=Windows`, then:

  ```powershell
  git add browser/script-loader/Cargo.toml browser/script-loader/src/lib.rs browser/script-loader/tests/script_loader.rs Cargo.lock
  git commit -m "feat: cache browser script preparation per page"
  git push origin HEAD
  ```

  Expected: a source-compatible page-session implementation is committed and synced.

## Task 3: Cached Browser Benchmark And Published Evidence

**Files:**
- Modify: `tools/benchmarks/Cargo.toml:9-12`
- Modify: `tools/benchmarks/src/lib.rs:12-365`
- Modify: `tools/benchmarks/tests/benchmarks.rs:1-45`
- Modify: `tools/demo/src/lib.rs`, `tools/demo/tests/demo.rs`
- Modify: `docs/performance.md`, `docs/benchmark-results.md`, `docs/roadmap.md`, `docs/milestones.md`, `README.md`

**Interfaces:**
- Consumes: `PageScriptSession::new`, `execute_inline_typescript`, and `cache_stats` from Task 2.
- Produces: `BenchmarkMode::CachedEntry`, whose `as_str()` is `"cached-entry"`.
- Produces: `BenchmarkResult { cache_hits: usize, cache_misses: usize, .. }`, with CSV header `name,mode,iterations,median_elapsed_micros,console_values,cache_hits,cache_misses`.
- Produces: `cached-page-startup` using `PAGE_ENTRY_SOURCE`, one session, one warm-up miss, fresh host/runtime state on each later cache-hit execution.

- [ ] **Step 1: Write failing benchmark contract tests**

  Extend `tools/benchmarks/tests/benchmarks.rs` so expected names and modes include:

  ```rust
  "cached-page-startup",
  BenchmarkMode::CachedEntry,
  ```

  For `run_default_benchmarks(2)`, assert six rows, console counts `[2, 2, 2, 0, 2, 2]`, zero cache counters for every non-cached row, and cached counters `cache_hits == 10`, `cache_misses == 1`. Extend the unit CSV test with `cache_hits: 5`, `cache_misses: 1` and expect:

  ```text
  name,mode,iterations,median_elapsed_micros,console_values,cache_hits,cache_misses
  page-startup,cold,1,42,1,5,1
  ```

- [ ] **Step 2: Run benchmark tests to verify they fail**

  Run:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-benchmarks'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: the scenario/mode and CSV assertions fail until cached-entry support exists.

- [ ] **Step 3: Implement cached-entry execution and observability**

  Add `tsvm-script-loader = { path = "../../browser/script-loader" }` to `tools/benchmarks/Cargo.toml`. Add `CachedEntry` to `BenchmarkMode` and add `cache_hits` / `cache_misses` fields to `BenchmarkResult`; initialize them to zero for cold, warm-entry, and warm-handler scenarios.

  In `run_benchmark`, create exactly one `PageScriptSession::new(1)` for `CachedEntry`. Run its unmeasured warm-up through `execute_inline_typescript(PAGE_ENTRY_SOURCE, &HostEnvironment::new(), ScriptPolicy::default())`, which records one miss. Use that same mutable session for every timed iteration, validate `console.log(150)`, and collect its final stats after all five measured samples. With `iterations == n`, report `cache_hits == MEASURED_SAMPLES * n` and `cache_misses == 1`; do not include cache creation or warm-up time in samples.

- [ ] **Step 4: Update the demo and focused verification**

  Update demo tests/output expectations only as needed for the expanded CSV header and sixth row. Run:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-benchmarks && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-demo && "C:\Users\Raman\.cargo\bin\cargo.exe" run -p tsvm-demo'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: benchmark tests prove counters and the demo shows the cached scenario without claiming browser-engine performance.

- [ ] **Step 5: Produce actual release benchmark evidence**

  Run the release command on the same Windows GNU toolchain and record its raw output:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" run --release -p tsvm-benchmarks -- 1000'
  & cmd.exe /d /s /c $cmd
  ```

  Update `docs/benchmark-results.md` with the exact command, date, Windows version, CPU, `rustc --version`, `release` profile, one warm-up plus five timed samples, all six raw CSV rows, and an explicit statement that cached hits come from the five timed samples while the single recorded miss came from the unmeasured warm-up.

- [ ] **Step 6: Update user-facing documentation**

  In `docs/performance.md`, document `cached-page-startup` / `cached-entry`, the three cache columns, page-owned/FIFO/exact-source behavior, and the lack of any real Chromium or cross-engine claim. In `README.md`, update status to M16 and add a short `PageScriptSession` example that constructs capacity `8`, executes an inline TypeScript script with `ScriptPolicy::default()`, and reads `cache_stats`.

  Add an M16 row to `docs/roadmap.md`, update its current summary and priorities, and add an M16 acceptance/deferred section to `docs/milestones.md`. State precisely that M16 caches verified preparation inside the standalone model and still defers Blink dispatch, browser cache partitioning/invalidation, events, timers, promises, async fetch, real DOM, and fair cross-engine comparisons.

- [ ] **Step 7: Run complete regression verification**

  Run these gates from the implementation worktree:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" fmt --all -- --check --config newline_style=Windows && "C:\Users\Raman\.cargo\bin\cargo.exe" clippy --workspace --all-targets -- -D warnings && "C:\Users\Raman\.cargo\bin\cargo.exe" test --workspace && "C:\Users\Raman\.cargo\bin\cargo.exe" run -p tsvm-corpus-runner && "C:\Users\Raman\.cargo\bin\cargo.exe" test -p tsvm-security-hardening'
  & cmd.exe /d /s /c $cmd
  ```

  Then build the C ABI and run the existing C++20 adapter smoke binary, remove only its generated object files after resolving their paths inside the worktree, and check `git diff --check`.

- [ ] **Step 8: Commit and publish M16**

  Stage only the M16 benchmark, demo, lockfile, documentation, and roadmap/milestone files. Then:

  ```powershell
  git commit -m "perf: publish cached browser script baseline"
  git push origin HEAD
  ```

  Expected: the remote contains reproducible benchmark evidence and documentation that accurately limits the claim to standalone TSVM behavior.

## Self-Review

**Spec coverage:** Task 1 covers bounded capacity, zero rejection, exact text keys, immutable verified entries, success-only insertion, deterministic FIFO, and all four cache counters. Task 2 covers page ownership, per-request policy ordering, caller hosts, fresh runtime state, inline and post-resolution external scripts, and one-shot compatibility. Task 3 covers the distinct cached benchmark mode, output validation, counters, release measurements, documentation, roadmap status, and full verification gates. Non-goals and security requirements are carried into global constraints and testing.

**Placeholder scan:** The plan contains no unresolved markers, unnamed APIs, unspecified error branches, or generic test instructions. Every implementation task names concrete files, public signatures, expected outcomes, and commands.

**Type consistency:** `PreparedModuleCache`, `PreparedModuleCacheError`, `PreparedModuleCacheStats`, `CacheLookup`, and `CacheLookupStatus` are defined in Task 1 and used with the same signatures in Tasks 2 and 3. `PageScriptSession`, `execute_inline_typescript`, and `cache_stats` are defined in Task 2 and consumed consistently by the benchmark task.
