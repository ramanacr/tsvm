# Architecture

TSVM is designed as a browser-hosted TypeScript runtime that is independent from
V8. JavaScript remains available through the browser's existing JavaScript
engine; TypeScript follows a separate pipeline.

```text
.ts source
  -> lexer
  -> parser
  -> typed AST
  -> semantic analyzer
  -> typed IR
  -> bytecode generator
  -> bytecode verifier
  -> interpreter
  -> browser capability layer
```

The current repository implements the lexer, shared AST, parser, semantic
analysis, typed IR, bytecode encoding/decoding, bytecode verifier,
verified-bytecode interpreter, managed heap, local module graph, and standalone
interop-boundary milestones, plus a standalone browser script-loader model.
Empty directories exist for later stages so code can land in the intended
ownership boundaries without reshaping the project each time.

## DOM And Fetch Bindings

`web-bindings/dom-fetch` exposes a minimal host document and same-origin fetch
service through `HostEnvironment`. TSVM code reaches DOM and fetch only by
calling registered host functions. The binding converts values through
`InteropValue`, mutates host-owned document state outside the TS heap, and
rejects cross-origin fetch URLs before response text enters the runtime.

The initial binding names are deliberately explicit (`domText`, `domSetText`,
and `fetchText`) so the current semantic analyzer can type-check them with
ordinary TypeScript stubs. Future browser work can replace those stubs with
ambient DOM declarations.

## Script Loading

`browser/script-loader` models the future Chromium hook for
`<script type="text/typescript">`. It scans a page for TypeScript script tags,
resolves external `.ts` resources against the document URL, executes external
scripts through the module graph and inline scripts directly through the
verified TSVM pipeline, and leaves ordinary JavaScript script tags untouched.
The loader records `generated_javascript = false` as an executable invariant for
tests.

This crate is intentionally not a complete HTML parser or browser shell. Its job
is to prove the runtime contract that the Chromium-side loader must preserve
once the C++ embed arrives.

## Interop Boundary

The standalone M9 interop layer models host integration without embedding a
JavaScript engine. `runtime/interop` defines explicit `InteropValue` variants
for primitives, objects, arrays, null, and undefined. Host functions are
registered by name in `HostEnvironment`; TSVM calls are converted from runtime
values into interop values, dispatched through the host registry, converted back
into heap-backed runtime values, and surfaced as `ExecuteError::Interop` on
host failure.

The reverse direction is handled by `PreparedModule`, which compiles and
verifies TS bytecode once, then lets host code call named TS functions with
`InteropValue` arguments. This gives browser embedding work a concrete value
and call model while keeping the standalone runtime free of V8 dependencies.

## Module Loading

The standalone runtime resolves local relative `.ts` imports through
`runtime/modules`. The module graph builder parses every reachable source file,
rejects unsupported specifiers and missing modules, detects cycles during DFS,
and emits modules in dependency-first order. For the current standalone runtime,
exports are unwrapped and imports are stripped into a deterministic bundled
source string that then enters the normal semantic-analysis, IR, bytecode,
verifier, and interpreter path.

This is deliberately not a browser loader yet. Network fetch, MIME checks, CSP,
origin policy, and module map integration belong to the browser binding
milestones.

## Heap And Runtime Values

Runtime objects and arrays are allocated in `runtime/heap` and referenced by
generation-checked handles. The first collector is intentionally non-moving:
slots can be reused after collection, but every reuse advances the generation so
old handles fail closed instead of resolving to a different object.

The interpreter keeps primitive values inline and stores aggregate values behind
heap handles. Before execution output crosses the host boundary, the
interpreter roots the return value and `console.log` arguments, runs collection,
and materializes those rooted values into plain API values. This models the
future browser boundary where DOM wrappers, JS interop values, and host-visible
objects must be explicit roots.

## Runtime Boundary

The future Chromium renderer owns TSVM instances. TSVM must not live in the
privileged browser process. DOM, fetch, console, timers, storage, and interop
must be exposed through capability-checked bindings that obey the browser's
same-origin policy, CSP, site isolation, and sandbox rules.

## Chromium C ABI Bridge

`runtime/c-api` gives Chromium-side C++ a stable native boundary without
introducing a JavaScript representation. The safe Rust core accepts UTF-8,
executes the existing interpreter pipeline, maps errors into ABI status values,
and emits deterministic tagged JSON. A thin FFI adapter contains the only raw
pointer conversions and catches panics before they can cross into C++.

`browser/chromium/tsvm_renderer_bridge` copies the result payload before freeing
its opaque Rust handle. It does not expose V8 values, DOM wrappers, network
objects, filesystem access, or browser IPC. The future Blink hook remains
responsible for CSP, origin, script policy, and renderer-process ownership.

See [`c-api.md`](c-api.md) for the public ownership and versioning contract.

## No JavaScript Fallback

The TypeScript execution path must never emit JavaScript as an intermediate or
fallback representation. Tests and docs should continue to prove that
TypeScript reaches execution only through typed IR, verified bytecode, and the
TSVM interpreter.
