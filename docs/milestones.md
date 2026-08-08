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

## M10-M13

See [`roadmap.md`](roadmap.md) for the full sequence from semantic analysis
through Chromium integration, browser bindings, hardening, and JIT research.
