# Roadmap

TSVM is intentionally staged so the standalone runtime earns trust before it is
embedded in a browser.

## Milestone Status

| Milestone | Status | Achieved | Remaining |
| --- | --- | --- | --- |
| M0 engineering baseline | Done | Cargo workspace, CI checks, docs, ADRs, and security notes are in place. | Production CI can add fuzz, sanitizer, and dependency-audit jobs. |
| M1 lexer | Done | V0.1 TypeScript subset tokenization, spans, comments, golden tests, and corpus runner are implemented. | Broader TypeScript lexical coverage can grow with language scope. |
| M2 parser | Done | Spanned AST and parser cover declarations, statements, expressions, classes, imports, exports, and recovery tests. | Advanced syntax such as decorators, JSX, namespaces, and complex type syntax remains deferred. |
| M3 semantic analyzer | Done | Symbols, scopes, simple structural types, function signatures, locals, calls, member access, returns, and diagnostics are implemented. | Rich TypeScript type-system behavior remains out of scope for this pass. |
| M4 typed IR | Done | Valid programs lower to typed IR with functions, blocks, instructions, value IDs, control flow, and source spans. | Optimizer-oriented IR passes are not implemented yet. |
| M5 bytecode and verifier | Done | Deterministic bytecode encoding/decoding, module metadata, verifier checks, and malformed-input tests are implemented. | Deeper CFG/type-state analysis and capability verification can be expanded. |
| M6 interpreter | Done | Verified bytecode executes primitives, objects, arrays, locals, member mutation, calls, branches, returns, and `console.log`; demo logs `150`. | Exception execution and full runtime class semantics are deferred. |
| M7 heap and garbage collection | Done | Managed allocation, generation-checked handles, tracing collection, stale-handle rejection, interpreter integration, and stress tests are implemented. | Moving GC and real browser/JS/DOM wrapper rooting remain deferred. |
| M8 modules | Done | Local relative `.ts` imports/exports, deterministic graph traversal, cycle/missing/unsupported diagnostics, and bundled execution are implemented. | Browser module maps, MIME checks, network loading, and package-style resolution remain deferred. |
| M9 JS interop | Standalone model done | `InteropValue`, host function registry, TS-to-host calls, host-to-TS calls, and host error propagation are implemented. | Real V8 integration, promises, proxies, symbols, getters/setters, typed arrays, and exception conversion remain deferred. |
| M10 minimal browser embed | Standalone model done | `text/typescript` script loading, inline/external TS execution, no-generated-JS proof, and policy blocking are implemented in a Rust model. | Real Chromium/Blink renderer embedding, MIME integration, DevTools, network service hooks, and C++ bridge remain deferred. |
| M11 DOM and fetch | Standalone model done | Host document text mutation, host document reads, same-origin text fetch, cross-origin blocking, and interop-bound binding calls are implemented. | Full DOM, events, timers, async fetch/promises, CSP-backed fetch/script loading, and browser network integration remain deferred. |
| M12 security hardening | Standalone boundary tests done | Verifier-gate, script-policy, remote-module, cross-origin fetch, malformed-bytecode, and no-generated-JS regression tests are implemented. | Coverage-guided fuzzing, sanitizers, real renderer sandbox tests, CSP integration, and crash corpus minimization remain deferred. |
| M13 performance and JIT research | Baseline done | Interpreter benchmark runner, checked-in benchmark results, performance docs, and JIT constraints are implemented. | Real profiling, trend storage, optimizer passes, and JIT prototypes remain deferred until the browser boundary matures. |
| M14 Chromium bridge foundation | Done | Stable Rust C ABI, opaque result ownership, panic containment, deterministic tagged output, C++20 renderer adapter, and C++ syntax CI coverage are implemented. | A Chromium checkout must link the static library and add the real Blink `text/typescript` dispatch hook after script policy checks. |

## Current Summary

The repository has completed the first standalone implementation pass from the
original specification. The end-to-end demo command proves:

```sh
cargo +stable-x86_64-pc-windows-gnu run -p tsvm-demo
```

- TypeScript is parsed and executed through the TSVM pipeline.
- No TypeScript-to-JavaScript execution path is used.
- Verified bytecode execution logs `150` for the initial demo.
- `text/typescript` script loading works in the standalone browser-loader model.
- DOM mutation and same-origin fetch work through explicit host bindings.
- Cross-origin fetch is blocked.

The largest remaining work is turning the standalone models into real browser
integration: Chromium renderer embedding, Blink/V8 boundary work,
browser-native CSP/origin enforcement, broader language coverage, production
fuzzing/sanitizers, and longer-term performance research. M14 establishes the
native Rust/C++ call boundary required for that work without claiming a browser
embed exists yet.

## Near-Term Priorities

- Obtain a Chromium checkout and use the M14 bridge from a Blink
  `text/typescript` dispatch hook after normal script policy checks.
- Replace host-function TypeScript stubs with ambient declarations for DOM,
  fetch, and interop APIs.
- Expand language support toward exceptions, classes, loops, async, and richer
  structural typing.
- Add CI jobs for benchmark trend capture, fuzz smoke, sanitizer builds, and
  dependency audits.
- Keep the interpreter baseline authoritative before any JIT research branch is
  allowed to execute generated code.

## Long-Term Browser Goal

The first meaningful browser success is a `.ts` script loaded with
`<script type="text/typescript">`, parsed directly as TypeScript, lowered to
typed IR and verified bytecode, executed by TSVM, and allowed to call
`console.log`, mutate DOM, and call `fetch` subject to normal browser security
rules.
