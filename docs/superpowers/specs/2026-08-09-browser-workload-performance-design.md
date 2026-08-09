# Browser-Workload Performance Design

## Purpose

M15 turns TSVM's M13 compile-and-run microbenchmark baseline into a useful
browser-workload performance program. It focuses on the browser lifecycle that
TSVM can exercise today: compile a TypeScript script, retain verified bytecode
for a page session, execute a warm entry point or handler, and make explicit
host-binding calls.

The goal is to make the interpreter fast for this workload without weakening
the runtime's central invariant:

```text
TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM
```

TSVM will not compile TypeScript to JavaScript, invoke a JavaScript engine as a
fallback, or claim to outperform every JavaScript renderer. Comparative browser
claims require a real Chromium integration and identical, published workloads.

## Scope

M15 has two tightly coupled deliverables:

1. A prepared-execution API that compiles and verifies source once, then runs
   the prepared program repeatedly without repeating frontend work.
2. A dependency-free benchmark harness that reports cold and warm
   browser-style workload measurements separately and verifies each workload's
   observable result.

The milestone will preserve the existing one-shot `execute_source` API. The
prepared API is additive and becomes the building block for a future renderer
page session.

## Non-Goals

M15 does not add a JIT, generated executable code, a TypeScript-to-JavaScript
path, real Chromium source changes, Blink hooks, DOM events, timers, promises,
or network integration. It also does not replace the current explicit host
bindings with a full DOM implementation.

Those features need a real browser boundary and their own security review. In
particular, event loops and asynchronous scheduling must not be simulated just
to produce a benchmark number.

## Architecture

### Prepared Program Boundary

The interpreter crate will expose a safe prepared-program value. Constructing
it performs the existing parse, semantic analysis, lowering, bytecode
encoding, decoding, and verification pipeline. Running it creates the required
execution state and executes the verified entry program without rebuilding the
pipeline.

The value owns immutable verified bytecode and module metadata. It must not
expose mutable bytecode or allow callers to bypass verifier gating. A new
execution receives a fresh runtime state unless a later, separately designed
page-session API makes state persistence explicit.

This separation makes two costs measurable and independently optimizable:

- **Cold script load:** source through verified execution.
- **Warm prepared execution:** fresh execution state over already verified
  bytecode.

The existing function-call facility may be used for a named synchronous handler
only when it preserves the same verifier and host-environment guarantees.

### Browser-Style Host Fixture

Benchmarks will use the current standalone browser bindings rather than mock
JavaScript. A small deterministic fixture will create a host environment with
document text and same-origin fetch resources. It will be reused only where the
workload mode says reuse is intended; otherwise each iteration receives a
fresh fixture.

The fixture represents the runtime-facing portion of a page, not a general DOM
or a browser scheduler. Every binding result used by a workload is asserted so
that an optimization cannot accidentally benchmark a skipped host call.

### Benchmark Runner

The benchmark crate will model a scenario as named source, workload mode,
fixture setup, iteration count, and an observable expectation. It will retain
the simple command-line entry point while evolving CSV output to include mode
and a robust timing field.

Each scenario runs one unmeasured warm-up followed by a fixed number of timed
samples. The reported value is the median elapsed microseconds for the supplied
iteration count. The runner will fail if execution fails or its expected
console and host-visible values do not match. It will never hide a failed
scenario behind a timing result.

The initial output shape is:

```text
name,mode,iterations,median_elapsed_micros,console_values
```

Documented results will state the operating system, Rust toolchain, Cargo
profile, CPU where available, sample count, and command. Results from different
machines are trend evidence, not directly comparable rankings.

## Initial Workloads

The initial suite is intentionally small and repeatable.

| Workload | Mode | What it exercises |
| --- | --- | --- |
| `page-startup` | cold | TypeScript frontend, verification, entry execution, and initial page state creation. |
| `prepared-page-entry` | warm | Repeated verified entry execution without recompilation. |
| `prepared-handler-dispatch` | warm | Named synchronous function invocation over prepared bytecode and ordinary object state. |
| `dom-binding-update` | warm | Explicit document text reads and writes through the existing host binding boundary. |
| `same-origin-fetch-update` | warm | Same-origin text fetch, TypeScript processing, and a host-visible document update. |

The workload source remains within TSVM's supported TypeScript subset. A
scenario that needs unsupported language features belongs to language-coverage
work, not the performance suite.

## Optimization Order

Implementation follows evidence instead of speculative engine work:

1. Add prepared execution and browser-style correctness fixtures.
2. Measure the cold and warm paths separately and publish the first M15
   baseline.
3. Profile the slowest warm workload on the supported development platform.
4. Optimize only a measured hot path, beginning with interpreter dispatch,
   binding lookup, and object/property access as profiling justifies it.
5. Re-run correctness, security, and benchmark checks after every optimization.

No JIT experiment may be introduced by this milestone. If future profiling
shows an interpreter limit, a JIT proposal must separately specify W^X memory,
renderer sandboxing, verifier enforcement, platform signing, deterministic
interpreter fallback, and browser-level benchmarks.

## Correctness And Security Rules

- Both one-shot and prepared execution use the same verified bytecode contract.
- Invalid source and malformed bytecode remain rejected before execution.
- Every benchmark scenario has deterministic expected observable output.
- Host calls continue to use explicit capabilities; same-origin and script
  policy checks are not weakened for measurement.
- The no-generated-JavaScript regressions remain required.
- Timing is never asserted as a hard CI pass/fail threshold because shared CI
  hardware is noisy. CI validates benchmark execution and result shape; human
  review evaluates published trends on documented hardware.

## Testing

Tests will cover prepared construction, repeated entry execution, repeated
handler execution where supported, fresh runtime state per execution, and
error propagation. Benchmark tests will verify scenario names, modes, expected
observable results, CSV headers, and median aggregation with deterministic
synthetic durations where practical.

Existing workspace tests, Clippy with warnings denied, lexer corpus tests,
documentation checks, C ABI tests, and the C++20 syntax gate remain part of the
milestone verification. The benchmark executable will run the new workload
suite before results are published.

## Published Results And Fair Comparison

The first M15 result file will be committed to the repository beside the M13
baseline. It will include the exact command, environment, raw scenario rows,
and an explanation of what cold and warm measurements include. A result is not
published until all benchmark scenarios complete with their expected values.

External comparisons are deferred until TSVM executes the same workload inside
real Chromium. That later comparison must disclose browser revision, TSVM
revision, operating system, CPU, flags, sample methodology, security mode, and
functional equivalence. It may report measured facts but must not generalize
them into a claim that TSVM beats all JavaScript engines or renderers.

## Definition Of Done

M15 is complete when:

1. The interpreter provides a documented, tested prepared-execution API.
2. The benchmark runner separates cold and warm modes and reports medians.
3. The five initial browser-style workloads execute through TSVM with checked
   observable results.
4. An M15 benchmark baseline is committed with reproducible environment and
   command details.
5. Documentation explains the lifecycle, result interpretation, and the
   boundaries of any performance claim.
6. The complete workspace and existing security/no-JavaScript checks pass.

## Follow-On Work

After M15, the next performance work depends on evidence: targeted interpreter
optimizations for measured warm-path hot spots, then a real Chromium page
session using the M14 C ABI bridge. Browser events, timers, async fetch, and
cross-engine comparisons become meaningful only in that real integration.
