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

- Deepen Chromium renderer integration from the standalone script-loader model.
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
