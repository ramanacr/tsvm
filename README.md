# TSVM

TSVM is a TypeScript-native browser runtime experiment. The project is building
a second scripting runtime for Chromium-based browsers where TypeScript is
parsed, analyzed, lowered, verified, and interpreted directly.

The core invariant is:

```text
TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM
```

There is no TypeScript-to-JavaScript execution path in the TSVM pipeline. Normal
JavaScript remains a browser feature through V8; TypeScript is treated as a
separate runtime with its own compiler pipeline, verifier, interpreter, heap,
and browser capability boundary.

## Current Status

This repository currently implements the M0-M7 standalone runtime foundation from
[`ts-browser-runtime-implementation.md`](ts-browser-runtime-implementation.md):

- Repository scaffold for browser integration, runtime stages, web bindings,
  interop, security, tests, and tools.
- Rust workspace with the first runtime crate, `tsvm-lexer`.
- V0.1 TypeScript lexer for keywords, identifiers, literals, punctuation,
  operators, comments, whitespace, and source spans.
- Golden lexer tests and valid/invalid fixture corpus.
- Spanned AST model and parser for V0.1 declarations, statements, expressions,
  object/array literals, type annotations, imports, exports, classes, and
  recoverable diagnostics.
- Parser fixture corpus for the initial demo and module/type syntax.
- Semantic analyzer for global symbols, scoped locals, simple structural type
  resolution, function signatures, local variable usage, calls, member access,
  returns, and source-spanned diagnostics.
- Semantic fixture corpus for valid and invalid TypeScript samples.
- Typed IR lowering for semantically valid programs, with functions, entry
  blocks, typed instructions, function calls, member access, control-flow
  branches, and source references.
- IR fixture corpus for the initial demo pipeline.
- Deterministic bytecode encoding and safe decoding with a stable version
  header, constant pool, function table, exception tables, source map
  references, and verifier-visible type tags.
- Mandatory bytecode verifier checks for headers, operand counts, constant
  references, local references, value references, jump targets, source
  references, exception table entries, terminators, and basic type states.
- Bytecode fixture corpus for compile, verify, encode, and decode roundtrips.
- Verified-bytecode interpreter with runtime values for primitives, objects,
  arrays, local variables, member access/mutation, function calls, arithmetic,
  branches, returns, and a host `console.log` capability.
- Managed heap with stable generation-checked handles, tracing collection,
  stale-handle rejection, and stress coverage for unreachable allocation churn.
- Interpreter object and array allocation through the managed heap, with
  return values and console output treated as cross-boundary roots.
- Initial demo execution proof: `.ts` source reaches parser, AST, semantic
  analysis, typed IR, verified bytecode, interpreter execution, and logs `150`.
- Interpreter fixture corpus for verified execution.
- Deterministic corpus smoke runner for fuzz-like CI coverage.
- Architecture, security, roadmap, milestone, and ADR documentation.

Chromium integration, modules, JS interop, DOM bindings, and fetch bindings are
intentionally not implemented yet. The implementation document starts with the
standalone pipeline so it can be proven before browser embedding begins.

## Repository Layout

```text
browser/                  Chromium shell, script loader, DevTools integration
runtime/                  TypeScript-native compiler and VM components
  lexer/                  Implemented M1 lexer crate
  ast/                    Implemented shared AST crate
  parser/                 Implemented M2 parser crate
  semantic/               Implemented M3 semantic analyzer crate
  ir/                     Implemented M4 typed intermediate representation
  bytecode/               Implemented M5 bytecode encoder/decoder/verifier
  verifier/               Future expanded verifier internals
  interpreter/            Implemented M6 verified-bytecode interpreter
  heap/                   Implemented M7 managed heap and tracing collector
web-bindings/             Future console, DOM, fetch, timers, events bindings
interop/                  Future JS/TS value and call boundary
security/                 Threat model, sandbox, CSP, and origin policy notes
tests/fixtures/lexer/     Valid and invalid lexer corpus
tests/fixtures/parser/    Valid parser corpus
tests/fixtures/semantic/  Valid and invalid semantic corpus
tests/fixtures/ir/        Valid IR lowering corpus
tests/fixtures/bytecode/  Valid bytecode corpus
tests/fixtures/interpreter/ Valid interpreter corpus
tools/                    Developer tools and corpus runners
docs/adr/                 Architecture decision records
```

## Requirements

- Rust 1.82 or newer.
- Cargo, rustfmt, and Clippy.

On Windows, the GNU Rust target can run this workspace without the MSVC linker:

```sh
rustup target add x86_64-pc-windows-gnu
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

## Common Commands

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p tsvm-lexer --bin lexer_corpus_runner -- tests/fixtures/lexer
```

## Lexer API

```rust
use tsvm_lexer::{lex, TokenKind};

let tokens = lex("const answer: number = 42;")?;
assert_eq!(tokens[0].kind, TokenKind::Const);
assert_eq!(tokens[1].span.start.line, 1);
```

The lexer returns tokens with byte offsets and one-based line/column spans.
Whitespace and comments are skipped. Invalid scanner input produces structured
diagnostics, including unterminated strings and block comments.

## Parser API

```rust
use tsvm_ast::StatementKind;
use tsvm_parser::parse_source;

let parsed = parse_source("const answer: number = 42;");
assert!(parsed.diagnostics.is_empty());
assert!(matches!(parsed.program.body[0].kind, StatementKind::Variable(_)));
```

The parser consumes lexer tokens and produces a spanned AST plus diagnostics.
Recoverable errors are represented in the AST so later valid statements can
still be parsed and reported.

## Semantic Analyzer API

```rust
use tsvm_semantic::{analyze_source, DiagnosticCode};

let analyzed = analyze_source("const answer: number = 42;");
assert!(analyzed.diagnostics.is_empty());
```

The semantic analyzer builds global type/value symbols, checks scoped local
variables, resolves simple structural object types, validates function calls and
returns, and reports diagnostics with source spans.

## Typed IR API

```rust
use tsvm_ir::{lower_source, IrInstructionKind};

let lowered = lower_source("const answer: number = 40 + 2;");
let ir = lowered.ir.expect("valid TypeScript should lower");
assert!(ir.entry.blocks[0]
    .instructions
    .iter()
    .any(|instruction| matches!(instruction.kind, IrInstructionKind::Binary { .. })));
```

IR lowering runs semantic analysis first. Invalid programs return diagnostics
and do not produce IR. Valid programs lower into entry/function IR with typed
instructions and source spans.

## Bytecode API

```rust
use tsvm_bytecode::{compile_source, encode_module, decode_module, verify_module};

let output = compile_source("const answer: number = 42;");
let module = output.module.expect("valid TypeScript should compile");
verify_module(&module)?;
let bytes = encode_module(&module);
let decoded = decode_module(&bytes)?;
assert_eq!(decoded, module);
```

The bytecode generator compiles verified IR into a deterministic module format.
The verifier treats compiler output as untrusted and must pass before any future
interpreter executes bytecode.

## Interpreter API

```rust
use tsvm_interpreter::{execute_source, Value};

let output = execute_source(r#"
function add(a: number, b: number): number {
  return a + b;
}
console.log(add(40, 2));
"#)?;

assert_eq!(output.console, vec![Value::Number(42.0)]);
```

The interpreter only executes modules that pass bytecode verification. Invalid
source returns semantic diagnostics before bytecode execution; malformed modules
return verifier errors before runtime state is created.

## Heap API

```rust
use tsvm_heap::{GcHeap, Trace, Tracer};

#[derive(Clone)]
struct Node(Vec<tsvm_heap::HeapHandle>);

impl Trace for Node {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for handle in &self.0 {
            tracer.mark(*handle);
        }
    }
}

let mut heap = GcHeap::new();
let child = heap.allocate(Node(Vec::new()));
let root = heap.allocate(Node(vec![child]));
let report = heap.collect([root]);
assert_eq!(report.marked, 2);
```

The heap uses non-moving slots plus generation-checked handles. Collection is a
simple tracing pass from explicit roots; unreachable slots are freed and reused
with a new generation so stale handles do not resolve. The interpreter uses the
heap for runtime objects and arrays, then roots return values and console output
before final materialization.

## V0.1 Lexer Scope

Supported token families:

- `let`, `const`, `var`
- primitive type keywords: `number`, `string`, `boolean`
- literal keywords: `true`, `false`, `null`, `undefined`
- functions, arrows, classes, interfaces, type aliases
- imports and exports for local modules
- `if`, `else`, `switch`, loops, exceptions, and returns
- identifiers, number literals, string literals
- core arithmetic, assignment, comparison, boolean, nullish, member, call, and
  delimiter tokens
- line comments and block comments

Deferred language work now starts in M8 with deterministic local module loading.

## Security Posture

TSVM treats every page, script, module, network response, decoded asset, and
renderer process as untrusted. The future runtime must execute inside the
renderer sandbox and may only reach browser capabilities through checked
bindings and browser IPC. Static TypeScript types are never trusted as proof
that external data is safe.

See:

- [`security/threat-model.md`](security/threat-model.md)
- [`security/sandbox.md`](security/sandbox.md)
- [`security/csp.md`](security/csp.md)
- [`security/origin-policy.md`](security/origin-policy.md)

## Roadmap

The next milestone is M8: deterministic local module loading with cycle
detection and clear diagnostics. The longer sequence continues with interop,
browser embedding, DOM/fetch, hardening, and performance research.

See [`docs/roadmap.md`](docs/roadmap.md) and
[`docs/milestones.md`](docs/milestones.md).

## Contributing

Keep the central invariant intact. Do not add a TypeScript-to-JavaScript
execution path, even for demos or tests. New runtime behavior should arrive with
tests first, fixture coverage when possible, and documentation for any boundary
that affects browser security.
