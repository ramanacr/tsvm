# Browser-Workload Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add verifier-gated prepared entry execution and a reproducible browser-style benchmark suite that reports separate cold and warm TSVM costs.

**Architecture:** `PreparedModule` remains the immutable owner of verified bytecode and gains public entry execution methods that create a fresh interpreter each time. The benchmark crate prepares source once only for warm scenarios, supplies a deterministic standalone browser host fixture for binding workloads, validates every observable result, and emits median timing rows. Documentation publishes the M15 baseline with the exact environment and command.

**Tech Stack:** Rust 1.82, Cargo workspace, standard-library timing, existing `tsvm-interpreter`, `tsvm-interop`, and `tsvm-web-bindings` crates.

## Global Constraints

- Preserve `TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM` for every scenario.
- Add no TypeScript-to-JavaScript execution path, JavaScript fallback, JIT, executable-code generation, Chromium source change, timer, promise, event-loop, or network feature.
- `PreparedModule` must keep its `BytecodeModule` private and must execute only bytecode verified during construction.
- One-shot execution remains source-compatible and uses the same compile-and-verify contract as prepared execution.
- A prepared execution creates fresh interpreter and heap state; host persistence is chosen explicitly by the caller.
- Benchmarks use no new third-party dependencies, run one unmeasured warm-up plus five timed samples, and report the sample median in microseconds.
- Every benchmark iteration checks its console, returned handler value, or browser host-visible state before contributing a timing row.
- Benchmark timing is not a hard CI threshold; CI validates correctness and result shape.
- Preserve the existing verifier, script-policy, same-origin, C ABI, C++20 syntax, and no-generated-JavaScript checks.
- Commit and push each completed milestone.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `runtime/interpreter/src/lib.rs` | Expose fresh-state entry execution over the already verified module held by `PreparedModule`; route one-shot source execution through it. |
| `runtime/interpreter/tests/interpreter.rs` | Prove prepared entry execution, host capability use, fresh state, and compile failure behavior. |
| `tools/benchmarks/Cargo.toml` | Add the existing standalone DOM/fetch binding crate to benchmark dependencies. |
| `tools/benchmarks/src/lib.rs` | Define benchmark modes, deterministic scenarios and fixtures, five-sample median aggregation, validation, and CSV row formatting. |
| `tools/benchmarks/src/main.rs` | Print the CSV header, print successful rows, and exit nonzero for benchmark correctness failures. |
| `tools/demo/src/lib.rs` | Render the new benchmark CSV shape and treat benchmark failure as a failed demo invariant. |
| `docs/performance.md` | Document cold/warm lifecycle semantics, sample methodology, commands, and interpretation boundaries. |
| `docs/benchmark-results.md` | Publish the measured M15 baseline with hardware and toolchain context. |
| `docs/roadmap.md` | Add M15 as an implemented performance milestone and update the remaining work description. |
| `docs/milestones.md` | Record M15 acceptance evidence and explicit deferrals. |
| `README.md` | Expose prepared execution and browser-workload benchmark availability to users. |

## Task 1: Prepared Entry Execution

**Files:**
- Modify: `runtime/interpreter/src/lib.rs:48-117`
- Modify: `runtime/interpreter/tests/interpreter.rs:1-213`

**Interfaces:**
- Consumes: `PreparedModule::from_source(source: &str) -> Result<PreparedModule, ExecuteError>`, `HostEnvironment`, `ExecutionOutput`.
- Produces: `PreparedModule::execute(&self) -> Result<ExecutionOutput, ExecuteError>` and `PreparedModule::execute_with_host(&self, host: &HostEnvironment) -> Result<ExecutionOutput, ExecuteError>`.
- Produces: source-compatible `execute_source_with_host` that delegates to `PreparedModule::from_source(source)?.execute_with_host(host)`.

- [ ] **Step 1: Add failing prepared-entry tests**

  Add the import and tests below to `runtime/interpreter/tests/interpreter.rs`.

  ```rust
  use tsvm_interpreter::{
      execute_module, execute_module_graph, execute_source, ExecuteError, PreparedModule, Value,
  };

  #[test]
  fn prepared_module_executes_verified_entry_with_host() {
      let prepared = PreparedModule::from_source(
          r#"
  function hostAdd(a: number, b: number): number { return 0; }
  console.log(hostAdd(20, 22));
  "#,
      )
      .expect("source should prepare");
      let host = HostEnvironment::new().with_function("hostAdd", host_add);

      let output = prepared
          .execute_with_host(&host)
          .expect("prepared entry should execute");

      assert_eq!(output.console, vec![Value::Number(42.0)]);
  }

  #[test]
  fn prepared_entry_execution_uses_fresh_runtime_state() {
      let prepared = PreparedModule::from_source(
          "const state = { count: 1 }; state.count += 1; console.log(state.count);",
      )
      .expect("source should prepare");

      let first = prepared.execute().expect("first execution should succeed");
      let second = prepared.execute().expect("second execution should succeed");

      assert_eq!(first.console, vec![Value::Number(2.0)]);
      assert_eq!(second.console, vec![Value::Number(2.0)]);
      assert_eq!(first.heap, second.heap);
  }

  #[test]
  fn prepared_module_refuses_invalid_source_before_execution() {
      let error = PreparedModule::from_source("const answer: number = \"bad\";")
          .expect_err("invalid source should not prepare");

      assert!(matches!(error, ExecuteError::Compile(_)));
  }
  ```

- [ ] **Step 2: Run the new tests and confirm the API is absent**

  Run:

  ```powershell
  cargo test -p tsvm-interpreter --test interpreter prepared_module_executes_verified_entry_with_host
  ```

  Expected: compilation fails because `PreparedModule::execute_with_host` and `PreparedModule::execute` do not exist.

- [ ] **Step 3: Implement the minimal prepared-entry API**

  In `runtime/interpreter/src/lib.rs`, replace the duplicated one-shot compile path with the existing `PreparedModule` construction, then add the methods shown below.

  ```rust
  pub fn execute_source_with_host(
      source: &str,
      host: &HostEnvironment,
  ) -> Result<ExecutionOutput, ExecuteError> {
      PreparedModule::from_source(source)?.execute_with_host(host)
  }

  impl PreparedModule {
      pub fn execute(&self) -> Result<ExecutionOutput, ExecuteError> {
          self.execute_with_host(&HostEnvironment::new())
      }

      pub fn execute_with_host(
          &self,
          host: &HostEnvironment,
      ) -> Result<ExecutionOutput, ExecuteError> {
          Interpreter::new(&self.module, host).execute()
      }
  }
  ```

  Keep `from_source` as the sole construction path, including its existing `verify_module` call. Do not expose `module` or add an unchecked constructor.

- [ ] **Step 4: Run focused interpreter regression tests**

  Run:

  ```powershell
  cargo test -p tsvm-interpreter --test interpreter
  cargo clippy -p tsvm-interpreter --all-targets -- -D warnings
  ```

  Expected: both commands pass, including prior verifier-gate and host-error tests.

- [ ] **Step 5: Commit the prepared-entry API**

  ```powershell
  git add runtime/interpreter/src/lib.rs runtime/interpreter/tests/interpreter.rs
  git commit -m "feat: execute prepared TSVM modules"
  git push origin main
  ```

## Task 2: Browser-Style Benchmark Harness

**Files:**
- Modify: `tools/benchmarks/Cargo.toml:8-9`
- Modify: `tools/benchmarks/src/lib.rs:1-89`
- Modify: `tools/benchmarks/src/main.rs:1-17`
- Modify: `tools/demo/src/lib.rs:5,145-158`

**Interfaces:**
- Consumes: `PreparedModule::execute_with_host`, `PreparedModule::call_function`, `BrowserBindings`, `Document`, `FetchService`, `InteropValue`, `Value`.
- Produces: `BenchmarkMode::{Cold,WarmEntry,WarmHandler}`, `BenchmarkResult`, `BenchmarkError`, `csv_header()`, `run_default_benchmarks(iterations) -> Result<Vec<BenchmarkResult>, BenchmarkError>`, and `BenchmarkResult::csv_row()`.
- Produces: five scenarios named `page-startup`, `prepared-page-entry`, `prepared-handler-dispatch`, `dom-binding-update`, and `same-origin-fetch-update`.

- [ ] **Step 1: Add the DOM/fetch binding dependency and failing benchmark tests**

  Add `tsvm-web-bindings = { path = "../../web-bindings/dom-fetch" }` under `[dependencies]` in `tools/benchmarks/Cargo.toml`.

  Add a `#[cfg(test)] mod tests` to `tools/benchmarks/src/lib.rs` with these tests:

  ```rust
  #[test]
  fn median_duration_returns_the_middle_sorted_sample() {
      let median = median_duration(vec![
          Duration::from_micros(90),
          Duration::from_micros(10),
          Duration::from_micros(50),
          Duration::from_micros(30),
          Duration::from_micros(70),
      ]);

      assert_eq!(median, Duration::from_micros(50));
  }

  #[test]
  fn default_browser_workloads_validate_at_one_iteration() {
      let results = run_default_benchmarks(1).expect("workloads should execute");
      let names = results.iter().map(|result| result.name.as_str()).collect::<Vec<_>>();

      assert_eq!(
          names,
          vec![
              "page-startup",
              "prepared-page-entry",
              "prepared-handler-dispatch",
              "dom-binding-update",
              "same-origin-fetch-update",
          ]
      );
      assert_eq!(results.len(), 5);
      assert!(results.iter().all(|result| result.median_elapsed >= Duration::ZERO));
  }

  #[test]
  fn csv_rows_include_the_documented_header_and_mode() {
      let result = BenchmarkResult {
          name: "page-startup".into(),
          mode: BenchmarkMode::Cold,
          iterations: 1,
          median_elapsed: Duration::from_micros(42),
          console_values: 1,
      };

      assert_eq!(csv_header(), "name,mode,iterations,median_elapsed_micros,console_values");
      assert_eq!(result.csv_row(), "page-startup,cold,1,42,1");
  }
  ```

- [ ] **Step 2: Run benchmark tests and confirm the new public surface is absent**

  Run:

  ```powershell
  cargo test -p tsvm-benchmarks
  ```

  Expected: compilation fails because the new modes, timing field, CSV helpers, median helper, and `Result`-returning runner do not exist.

- [ ] **Step 3: Implement deterministic scenarios and measurement**

  Replace `tools/benchmarks/src/lib.rs` with focused benchmark infrastructure using these exact public names:

  ```rust
  pub const MEASURED_SAMPLES: usize = 5;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum BenchmarkMode {
      Cold,
      WarmEntry,
      WarmHandler,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct BenchmarkResult {
      pub name: String,
      pub mode: BenchmarkMode,
      pub iterations: usize,
      pub median_elapsed: Duration,
      pub console_values: usize,
  }

  pub fn csv_header() -> &'static str;
  pub fn run_default_benchmarks(
      iterations: usize,
  ) -> Result<Vec<BenchmarkResult>, BenchmarkError>;
  ```

  Implement `BenchmarkMode::as_str()` with `cold`, `warm-entry`, and
  `warm-handler`. Implement `BenchmarkResult::csv_row()` using
  `median_elapsed.as_micros()` and the documented five-column order.

  Use a private `BenchmarkScenario` plus a private `ScenarioExpectation` to
  keep scenario source, mode, host setup, and observable checks together.
  `run_default_benchmarks` must reject `iterations == 0` with
  `BenchmarkError::InvalidIterations`, run one unmeasured scenario iteration,
  collect exactly `MEASURED_SAMPLES` elapsed durations, call
  `median_duration`, and return only validated results.

  Use these sources and observables:

  ```typescript
  // page-startup and prepared-page-entry
  interface Account { id: number; balance: number; }
  function credit(account: Account, amount: number): number {
    account.balance += amount;
    return account.balance;
  }
  const account: Account = { id: 1, balance: 100 };
  console.log(credit(account, 50));
  // expected console: 150

  // prepared-handler-dispatch
  function handleClick(current: number): number {
    const state = { clicks: current };
    state.clicks += 1;
    return state.clicks;
  }
  // call with 41; expected InteropValue::Number(42.0)

  // dom-binding-update
  function domText(selector: string): string { return ""; }
  function domSetText(selector: string, text: string): undefined { return undefined; }
  domSetText("#app", "updated by TSVM");
  console.log(domText("#app"));
  // expected console and document text: "updated by TSVM"

  // same-origin-fetch-update
  function domText(selector: string): string { return ""; }
  function domSetText(selector: string, text: string): undefined { return undefined; }
  function fetchText(url: string): string { return ""; }
  domSetText("#app", fetchText("/message.txt"));
  console.log(domText("#app"));
  // expected console and document text: "hello from TSVM"
  ```

  Construct browser bindings with document selector `#app`, origin
  `https://example.test`, and resource `/message.txt` containing
  `hello from TSVM`. Build a fresh `BrowserBindings` per binding-workload
  iteration so the document assertion is independent. Construct a prepared
  module once before warm-up for each warm scenario; cold scenarios call
  `execute_source` inside every iteration. Validate each iteration before
  recording its elapsed duration.

  Define `BenchmarkError` with variants for invalid iterations, TSVM execution,
  and failed expectations. Derive `Debug`; the binary may print errors with
  `{error:?}`. Convert `ExecuteError` with `From<ExecuteError>`.

- [ ] **Step 4: Make CLI and demo output consume successful benchmark results**

  In `tools/benchmarks/src/main.rs`, print `csv_header()` before rows. Match on
  `run_default_benchmarks(iterations)`: print `csv_row()` for each result on
  success; print `benchmark failed: {error:?}` to stderr and call
  `std::process::exit(1)` on error.

  In `tools/demo/src/lib.rs`, change the snapshot heading to `csv_header()` and
  call `run_default_benchmarks(3).expect("benchmark snapshot should execute")`.
  Render each result with `csv_row()` so the demo and standalone benchmark use
  the identical schema.

- [ ] **Step 5: Run benchmark, demo, and lint verification**

  Run:

  ```powershell
  cargo test -p tsvm-benchmarks
  cargo run -p tsvm-benchmarks -- 1
  cargo run -p tsvm-demo
  cargo clippy -p tsvm-benchmarks -p tsvm-demo --all-targets -- -D warnings
  ```

  Expected: five CSV rows follow the header, all five scenarios execute, the
  demo includes the same header, and lint passes with no warnings.

- [ ] **Step 6: Commit the benchmark harness**

  ```powershell
  git add tools/benchmarks/Cargo.toml tools/benchmarks/src/lib.rs tools/benchmarks/src/main.rs tools/demo/src/lib.rs
  git commit -m "feat: benchmark browser-style TSVM workloads"
  git push origin main
  ```

## Task 3: Publish M15 Results And Documentation

**Files:**
- Modify: `docs/performance.md:1-35`
- Modify: `docs/benchmark-results.md:1-30`
- Modify: `docs/roadmap.md:5-43`
- Modify: `docs/milestones.md:224-245`
- Modify: `README.md:15-80,159-175,386-415`

**Interfaces:**
- Consumes: the benchmark CSV header and five default scenario names from Task 2.
- Produces: reproducible M15 result record, updated user-facing commands, prepared-execution example, and accurate roadmap status.

- [ ] **Step 1: Update documentation with the new lifecycle and command contract**

  In `docs/performance.md`, replace the M13-only scenario list with the five
  M15 workloads. State precisely that `cold` measures compile, verify, and
  execution per iteration; `warm-entry` measures fresh execution from retained
  verified bytecode; and `warm-handler` measures a named host-to-TS function
  call from retained verified bytecode. Document one warm-up, five timed
  samples, median reporting, no CI timing threshold, and the requirement to
  publish environment details.

  In `README.md`, change current status to M0-M15, add a short prepared-entry
  example, and update the benchmark description to name the cold/warm browser
  workloads. Keep the no-JavaScript invariant and real-Chromium limitation
  prominent.

  In `docs/roadmap.md` and `docs/milestones.md`, add M15 as implemented only
  after all acceptance checks in this task pass. State that profiler-led hot
  path work and real Chromium page sessions remain deferred; do not describe
  M15 as a cross-engine comparison.

- [ ] **Step 2: Run the release-profile benchmark and capture the output**

  Run the command appropriate to the installed target. On this Windows setup:

  ```powershell
  cargo +stable-x86_64-pc-windows-gnu run --release -p tsvm-benchmarks -- 1000
  ```

  Record the exact command, date, operating system, Rust version from
  `rustc +stable-x86_64-pc-windows-gnu --version`, Cargo profile, measured CSV
  header and five rows, `MEASURED_SAMPLES = 5`, and CPU model from
  `Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Name`.

- [ ] **Step 3: Publish the measured M15 baseline**

  Replace the M13-only table in `docs/benchmark-results.md` with two clearly
  labeled sections: retain the historical M13 development-profile baseline,
  then add the newly captured M15 release-profile baseline. Explain that M13
  is complete compile-and-execute timing with three scenarios, while M15 uses
  cold/warm modes and medians. Copy only the actual output produced in Step 2;
  do not estimate, round, or fabricate timing values.

- [ ] **Step 4: Run documentation and full regression verification**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo run -p tsvm-lexer --bin lexer_corpus_runner -- tests/fixtures/lexer
  git diff --check
  ```

  Run the C++20 syntax and link smoke test inside the Visual Studio developer
  environment:

  ```powershell
  $cmd = 'call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 >nul && "C:\Users\Raman\.cargo\bin\cargo.exe" build -p tsvm-c-api && cl /nologo /std:c++20 /EHsc /I browser\chromium /I runtime\c-api\include browser\chromium\tsvm_renderer_bridge.cc browser\chromium\renderer_bridge_smoke.cc target\debug\tsvm_c_api.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib /Fe:target\debug\tsvm_renderer_bridge_smoke.exe && target\debug\tsvm_renderer_bridge_smoke.exe'
  & cmd.exe /d /s /c $cmd
  ```

  Expected: all Rust checks, corpus runner, documentation diff check, C API
  link smoke, and C++ executable pass. Remove only the known root-level
  temporary object files if the compiler produces them; verify their resolved
  paths are inside `C:\Work\tsvm` before removal.

- [ ] **Step 5: Commit and publish M15 results**

  ```powershell
  git add README.md docs/performance.md docs/benchmark-results.md docs/roadmap.md docs/milestones.md
  git commit -m "docs: publish browser workload benchmark baseline"
  git push origin main
  ```

## Final Milestone Verification

- [ ] **Step 1: Confirm clean synchronization and inspect the published diff**

  Run:

  ```powershell
  git status --short --branch
  git log -3 --oneline
  git show --stat --oneline HEAD
  ```

  Expected: `main...origin/main` has no pending changes, the three M15 commits
  are present, and the last commit contains only M15 documentation and results.

- [ ] **Step 2: Verify the pushed GitHub Actions run**

  Open the repository Actions page and confirm the workflow for the final M15
  commit completes successfully. Record the run URL in the final milestone
  report.

- [ ] **Step 3: Report the milestone accurately**

  State that M15 adds standalone prepared execution and browser-style workload
  measurements. State that this is not a real Chromium embed and is not an
  engine-wide performance claim. Link the baseline, performance documentation,
  roadmap, and final GitHub Actions run.
