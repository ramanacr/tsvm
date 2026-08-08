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
analysis, typed IR, bytecode encoding/decoding, bytecode verifier, and
verified-bytecode interpreter milestones. Empty directories exist for later
stages so code can land in the intended ownership boundaries without reshaping
the project each time.

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
