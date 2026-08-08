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

- Add DOM binding traits for querying and mutating a host document model.
- Add fetch binding traits that preserve same-origin checks before network-like
  responses enter TSVM.
- Preserve verifier-first execution before DOM/fetch host capabilities are
  exposed.
- Expand fixtures into browser binding tests for DOM mutation and fetch.
- Add coverage-guided fuzzing once the Rust toolchain and CI environment can
  support `cargo-fuzz`.

## Long-Term Browser Goal

The first meaningful browser success is a `.ts` script loaded with
`<script type="text/typescript">`, parsed directly as TypeScript, lowered to
typed IR and verified bytecode, executed by TSVM, and allowed to call
`console.log`, mutate DOM, and call `fetch` subject to normal browser security
rules.
