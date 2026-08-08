# Roadmap

TSVM is intentionally staged so the standalone runtime earns trust before it is
embedded in a browser.

1. M0 engineering baseline.
2. M1 lexer.
3. M2 parser.
4. M3 semantic analyzer.
5. M4 typed IR.
6. M5 bytecode and verifier.
7. M6 interpreter.
8. M7 heap and garbage collection.
9. M8 modules.
10. M9 JS interop.
11. M10 minimal browser embed.
12. M11 DOM and fetch.
13. M12 security hardening.
14. M13 performance and JIT research.

## Near-Term Priorities

- Replace the current reference-counted object handles with an explicit heap
  abstraction that can evolve toward tracing GC.
- Add allocation APIs, roots, handle types, and stress tests for object and
  array lifetimes.
- Preserve verifier-first execution while moving runtime objects behind managed
  handles.
- Expand fixtures into a shared corpus used by heap/GC, module, and interop
  tests.
- Add coverage-guided fuzzing once the Rust toolchain and CI environment can
  support `cargo-fuzz`.

## Long-Term Browser Goal

The first meaningful browser success is a `.ts` script loaded with
`<script type="text/typescript">`, parsed directly as TypeScript, lowered to
typed IR and verified bytecode, executed by TSVM, and allowed to call
`console.log`, mutate DOM, and call `fetch` subject to normal browser security
rules.
