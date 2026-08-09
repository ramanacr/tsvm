# Milestones

## M0: Engineering Baseline

Status: implemented in this scaffold.

Acceptance evidence:

- Cargo workspace exists at the repository root.
- Runtime, browser, binding, interop, security, tests, tools, and docs
  directories are present.
- CI runs formatting, linting, tests, corpus smoke, and documentation checks.
- ADR directory exists with the initial decision records.
- Threat model and security policy notes are drafted.

## M1: Lexer

Status: implemented.

Acceptance evidence:

- `runtime/lexer` tokenizes the V0.1 TypeScript subset.
- Tokens include byte, line, and column spans.
- Whitespace, line comments, and block comments are handled.
- Golden integration tests live in `runtime/lexer/tests/golden.rs`.
- Valid and invalid corpus fixtures live in `tests/fixtures/lexer`.
- `lexer_corpus_runner` provides a deterministic fuzz-like smoke target.

## M2: Parser

Status: implemented.

Acceptance evidence:

- `runtime/ast` defines the spanned AST model.
- `runtime/parser` parses V0.1 syntax into that AST.
- Parser diagnostics preserve later statements after common syntax errors.
- Tests cover declarations, control flow, expressions, classes,
  imports, exports, and error recovery.
- Fixture coverage lives in `tests/fixtures/parser`.

## M3: Semantic Analyzer

Status: implemented.

Acceptance evidence:

- `runtime/semantic` builds global symbols and scoped locals.
- Interfaces, type aliases, classes, and annotations resolve to simple semantic
  types.
- Function calls validate arity and argument assignability.
- Return statements validate declared return types.
- Local variable initializers validate annotations.
- Diagnostics include source spans and stable diagnostic codes.
- Valid and invalid corpus fixtures live in `tests/fixtures/semantic`.

## M4: Typed IR

Status: implemented.

Acceptance evidence:

- `runtime/ir` lowers semantically valid programs into typed IR.
- Invalid semantic programs return diagnostics and do not produce IR.
- IR functions contain typed params, return types, basic blocks, instructions,
  source spans, and value IDs.
- The initial demo lowers to an entry function plus a `credit` function.
- Control-flow tests produce branch and jump blocks.
- Fixture coverage lives in `tests/fixtures/ir`.

## M5: Bytecode And Verifier

Status: implemented.

Acceptance evidence:

- `runtime/bytecode` emits deterministic bytecode modules from typed IR.
- Bytecode modules include a stable header, constant pool, function table,
  exception table slots, source map references, and verifier-visible type tags.
- Encoder output is deterministic and decoder roundtrips valid modules.
- Decoder rejects malformed headers and invalid binary content.
- Verifier rejects bad constant references, value references, jump targets,
  source references, exception entries, missing terminators, and invalid type
  states.
- Fixture coverage lives in `tests/fixtures/bytecode`.

## M6: Interpreter

Status: implemented.

Acceptance evidence:

- `runtime/interpreter` refuses to execute modules that fail bytecode
  verification.
- `execute_source` compiles TypeScript through semantic analysis, typed IR,
  bytecode, verifier, and interpreter without generating JavaScript.
- Runtime values support numbers, strings, booleans, null, undefined, objects,
  and arrays.
- Interpreter supports local variables, member access, member mutation,
  arithmetic, function calls, branches, jumps, returns, and host `console.log`.
- The initial demo logs `150` through verified bytecode execution.
- Fixture coverage lives in `tests/fixtures/interpreter`.

Deferred from M6:

- Exceptions are parsed and represented, but full exception table execution is
  deferred to a later hardening pass.
- Classes lower structurally today; constructor/runtime class semantics are
  deferred until object model work matures.

## M7: Heap And GC

Status: implemented.

Acceptance evidence:

- `runtime/heap` provides managed allocation through generation-checked
  `HeapHandle` values.
- The collector traces explicit roots and collects unreachable objects.
- Stale handles fail to resolve after slot reuse.
- The interpreter allocates runtime objects and arrays through the managed heap.
- Return values and host `console.log` arguments are treated as cross-boundary
  roots before output materialization.
- Stress tests cover large volumes of unreachable allocations.

## M8: Modules

Status: implemented.

Acceptance evidence:

- `runtime/modules` resolves local relative `.ts` module imports.
- Module graph traversal is dependency-first and deterministic.
- Missing modules, unsupported specifiers, parser errors, and cycles produce
  structured diagnostics.
- Exports are collected from interfaces, type aliases, classes, functions, and
  variables.
- `execute_module_graph` compiles bundled module sources through the existing
  semantic analyzer, typed IR, bytecode verifier, interpreter, and heap path.
- Valid and invalid module fixtures live in `tests/fixtures/modules`.

## M9: JS Interop

Status: implemented for the standalone host boundary.

Acceptance evidence:

- `runtime/interop` defines explicit boundary values for primitives, objects,
  arrays, `null`, and `undefined`.
- Host functions can be registered in `HostEnvironment`.
- TSVM code can call registered host functions by name after semantic checking
  with a TypeScript stub.
- Host code can prepare a verified TS module and call named TS functions with
  `InteropValue` arguments.
- Host failures are reported as `ExecuteError::Interop` before they can be
  mistaken for verifier or semantic errors.
- Valid interop fixtures live in `tests/fixtures/interop`.

Deferred from M9:

- Real V8 integration, promises, proxies, symbols, getters/setters, and
  exception conversion are deferred until browser embedding introduces the
  actual JavaScript engine boundary.

## M10: Minimal Browser Embed

Status: implemented as a standalone script-loader model.

Acceptance evidence:

- `browser/script-loader` recognizes `<script type="text/typescript">`.
- External `.ts` scripts are resolved from page resources and executed through
  the module graph, semantic analyzer, typed IR, verified bytecode,
  interpreter, and heap.
- Inline TypeScript scripts execute through the same TSVM path.
- Normal JavaScript scripts are ignored by the TypeScript loader and left for
  the future browser/V8 path.
- Tests assert `generated_javascript` remains false for the TypeScript path.
- Browser script-loader fixtures live in `tests/fixtures/browser`.

Deferred from M10:

- A real Chromium shell, Blink integration, MIME handling, network service
  integration, DevTools hooks, and renderer-process embedding are deferred until
  the C++ integration phase.

## M11: DOM And Fetch

Status: implemented for the standalone host binding model.

Acceptance evidence:

- `web-bindings/dom-fetch` exposes a host-owned document model.
- TypeScript can mutate host document text through `domSetText`.
- TypeScript can read host document text through `domText`.
- TypeScript can fetch same-origin text resources through `fetchText`.
- Cross-origin fetch URLs are blocked before response data enters TSVM.
- Binding calls cross the same explicit interop boundary as M9.
- DOM/fetch fixtures live in `tests/fixtures/web-bindings`.

Deferred from M11:

- Full DOM APIs, event dispatch, timers, async fetch promises, browser network
  service integration, and CSP-enforced resource loading are deferred to
  security hardening and browser integration work.

## M12: Security Hardening

Status: implemented for standalone runtime boundaries.

Acceptance evidence:

- `security/hardening` runs executable security regression tests in the Cargo
  workspace.
- Interpreter execution is still gated by bytecode verification.
- Script policy can block TypeScript before compilation.
- Remote module specifiers are rejected by the module graph.
- Cross-origin fetch calls are rejected before response data enters TSVM.
- Malformed bytecode crash corpus inputs decode as errors.
- The TypeScript script-loader path continues to expose a no-generated-JS
  invariant.

Deferred from M12:

- Coverage-guided fuzzing, sanitizer builds, real renderer sandbox tests, CSP
  integration with Chromium, and crash corpus minimization are deferred until
  the browser integration environment exists.

## M13: Performance And JIT Research

Status: implemented for interpreter benchmarking and JIT constraints.

Acceptance evidence:

- `tools/benchmarks` provides a dependency-free interpreter benchmark runner.
- Default scenarios cover the initial demo, function calls, and object mutation.
- Benchmark output includes scenario name, iteration count, elapsed time, and
  stable console-value counts.
- `docs/performance.md` documents JIT research constraints, including W^X,
  platform code signing, renderer sandboxing, verifier gating, interpreter
  fallback, and the no-TypeScript-to-JavaScript invariant.
- ADR-0010 records continued JIT deferral.

Deferred from M13:

- Real profiling integration, benchmark trend storage, optimizer passes, and JIT
  prototypes are deferred until the browser/runtime boundary is mature enough to
  evaluate them safely.

See [`roadmap.md`](roadmap.md) for the full sequence from semantic analysis
through Chromium integration, browser bindings, hardening, and JIT research.

## M14: Chromium Bridge Foundation

Status: implemented.

Acceptance evidence:

- `runtime/c-api` exposes a versioned C ABI for length-delimited UTF-8
  TypeScript source.
- The ABI executes through the verified TSVM pipeline and records
  `generated_javascript: false` in its deterministic result envelope.
- Result allocation, borrowing, and release are owned by Rust through an opaque
  `tsvm_result` handle; panics are contained before crossing the C++ boundary.
- `browser/chromium/tsvm_renderer_bridge` provides a C++20 RAII adapter that
  copies result bytes before freeing the Rust handle.
- Rust ABI tests and a CI C++ syntax check cover the public boundary.

Deferred from M14:

- Chromium checkout linkage, Blink script dispatch, renderer sandbox tests,
  DevTools, V8 interop, and browser-owned DOM/fetch capability bindings remain
  real browser-integration milestones.

## M15: Browser-Workload Performance

Status: implemented for standalone prepared execution and browser-style
workload measurement.

Acceptance evidence:

- `PreparedModule` owns private verified bytecode and can execute the verified
  entry program repeatedly with a fresh interpreter and heap for each run.
- One-shot source execution shares the prepared module's compile-and-verify
  contract.
- `tools/benchmarks` separates cold source execution, warm prepared entry
  execution, and warm host-to-TS handler dispatch.
- Browser-style benchmark fixtures exercise explicit DOM text bindings and
  same-origin text fetch without adding an event loop or a browser network
  stack.
- Each benchmark iteration validates console output, handler results, or
  host-visible document text before timing is accepted.
- The runner performs one warm-up and five timed samples, reports a median CSV
  row, and exits nonzero on a failed expectation.
- `docs/benchmark-results.md` contains the M15 release-profile baseline with
  command, processor, operating system, Rust version, sample count, and raw
  rows.

Deferred from M15:

- Profiling-led interpreter dispatch, object-layout, and host-binding
  optimizations.
- Real Chromium renderer page sessions over the M14 C ABI bridge.
- Browser events, timers, promises, async fetch, and a full DOM.
- Cross-engine comparisons or claims about outperforming browser renderers.
