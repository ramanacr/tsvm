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
verified-bytecode interpreter, managed heap, and local module graph milestones.
Empty directories exist for later stages so code can land in the intended
ownership boundaries without reshaping the project each time.

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

## No JavaScript Fallback

The TypeScript execution path must never emit JavaScript as an intermediate or
fallback representation. Tests and docs should continue to prove that
TypeScript reaches execution only through typed IR, verified bytecode, and the
TSVM interpreter.
