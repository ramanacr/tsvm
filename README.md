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

This repository currently implements the M0-M13 standalone runtime foundation from
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
- Deterministic local module graph resolver for relative `.ts` imports and
  exports, including dependency-first ordering, cycle detection, missing-module
  diagnostics, unsupported-specifier diagnostics, and bundled execution through
  the verified TSVM pipeline.
- JS/TS interop boundary model for standalone host integration: explicit
  `InteropValue` conversion, registered host functions callable from TSVM, host
  calls into prepared TS functions, and boundary error propagation.
- Minimal browser script-loader model that recognizes
  `<script type="text/typescript">`, resolves local `.ts` script resources,
  executes external and inline TypeScript through TSVM, ignores normal
  JavaScript scripts, and records that no JavaScript is generated for the TS
  path.
- DOM/fetch binding model that exposes stateful host document mutation and
  same-origin text fetch through explicit interop host functions.
- Security hardening tests for verifier-gated execution, script policy
  blocking, remote module rejection, cross-origin fetch rejection, malformed
  bytecode crash corpus handling, and no-JavaScript TypeScript execution.
- Interpreter benchmark runner for core runtime paths plus documented JIT
  research constraints around W^X, code signing, verifier gating, sandboxing,
  and deterministic interpreter fallback.
- Initial demo execution proof: `.ts` source reaches parser, AST, semantic
  analysis, typed IR, verified bytecode, interpreter execution, and logs `150`.
- Interpreter fixture corpus for verified execution.
- Deterministic corpus smoke runner for fuzz-like CI coverage.
- Architecture, security, roadmap, milestone, and ADR documentation.

Full Chromium integration is intentionally not implemented yet. The
implementation document starts with the standalone pipeline so it can be proven
before browser embedding begins.

## Repository Layout

```text
browser/                  Chromium shell, script loader, DevTools integration
  script-loader/          Implemented M10 standalone TypeScript script loader
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
  modules/                Implemented M8 local module graph resolver
  interop/                Implemented M9 standalone host interop values
web-bindings/             Future console, DOM, fetch, timers, events bindings
  dom-fetch/              Implemented M11 DOM text and fetch host bindings
interop/                  Future JS/TS value and call boundary
security/                 Threat model, sandbox, CSP, and origin policy notes
  hardening/              Implemented M12 executable security regression tests
tests/fixtures/lexer/     Valid and invalid lexer corpus
tests/fixtures/parser/    Valid parser corpus
tests/fixtures/semantic/  Valid and invalid semantic corpus
tests/fixtures/ir/        Valid IR lowering corpus
tests/fixtures/bytecode/  Valid bytecode corpus
tests/fixtures/interpreter/ Valid interpreter corpus
tests/fixtures/modules/   Valid and invalid local module corpus
tests/fixtures/interop/   Valid interop boundary corpus
tests/fixtures/browser/   Valid script-loader browser fixture corpus
tests/fixtures/web-bindings/ Valid DOM/fetch binding corpus
tests/fixtures/security/ Crash and policy regression corpus
tools/                    Developer tools and corpus runners
  benchmarks/             Implemented M13 interpreter benchmark runner
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
cargo run -p tsvm-benchmarks -- 100
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

## Module Graph API

```rust
use std::collections::BTreeMap;
use tsvm_interpreter::{execute_module_graph, Value};

let sources = BTreeMap::from([
    ("/app.ts".into(), r#"import { answer } from "./answer.ts";
console.log(answer);"#.into()),
    ("/answer.ts".into(), "export const answer: number = 42;".into()),
]);

let output = execute_module_graph("/app.ts", &sources)?;
assert_eq!(output.console, vec![Value::Number(42.0)]);
```

M8 supports local relative `.ts` module specifiers such as `./account.ts` and
`../shared/math.ts`. Modules are parsed directly as TypeScript, ordered with a
deterministic dependency-first graph walk, bundled without import/export syntax,
and then compiled through semantic analysis, typed IR, verified bytecode, and
the interpreter. Non-local specifiers and cycles are rejected before
compilation.

## Interop API

```rust
use tsvm_interop::{HostEnvironment, InteropError, InteropValue};
use tsvm_interpreter::{execute_source_with_host, PreparedModule};

fn host_add(args: &[InteropValue]) -> Result<InteropValue, InteropError> {
    match args {
        [InteropValue::Number(left), InteropValue::Number(right)] => {
            Ok(InteropValue::Number(left + right))
        }
        _ => Err(InteropError::new("expected two numbers")),
    }
}

let host = HostEnvironment::new().with_function("hostAdd", host_add);
let output = execute_source_with_host(r#"
function hostAdd(a: number, b: number): number {
  return 0;
}
console.log(hostAdd(20, 22));
"#, &host)?;

let module = PreparedModule::from_source(
    "function add(a: number, b: number): number { return a + b; }",
)?;
let value = module.call_function(
    "add",
    &[InteropValue::Number(20.0), InteropValue::Number(22.0)],
    &HostEnvironment::new(),
)?;
```

M9 is a standalone interop boundary, not a browser JS engine embed. Host
functions are registered explicitly, arguments and return values cross through
`InteropValue`, and host failures become `ExecuteError::Interop`. Until ambient
declaration parsing exists, TS-callable host functions use TypeScript function
stubs for semantic checking; the host registry overrides matching names at
runtime.

## Browser Script Loader API

```rust
use std::collections::BTreeMap;
use tsvm_script_loader::execute_typescript_scripts;

let html = r#"<script type="text/typescript" src="/app.ts"></script>"#;
let resources = BTreeMap::from([
    ("/app.ts".into(), "console.log(42);".into()),
]);

let output = execute_typescript_scripts("/index.html", html, &resources)?;
assert!(!output.generated_javascript);
```

M10 recognizes `text/typescript` scripts and executes only those entries through
the TSVM pipeline. External `.ts` scripts can use the M8 module graph resolver;
inline TypeScript is compiled directly. Normal JavaScript script tags are left
alone for a future browser/V8 integration path.

## DOM And Fetch Binding API

```rust
use std::collections::BTreeMap;
use tsvm_interpreter::execute_source_with_host;
use tsvm_web_bindings::{BrowserBindings, Document, FetchService};

let bindings = BrowserBindings::new(
    Document::from_text_nodes([("#app", "")]),
    FetchService::new(
        "https://example.test",
        BTreeMap::from([("/message.txt".into(), "hello".into())]),
    ),
);

let source = r##"
function domSetText(selector: string, text: string): undefined { return undefined; }
function fetchText(url: string): string { return ""; }
domSetText("#app", fetchText("/message.txt"));
"##;
execute_source_with_host(source, &bindings.host_environment())?;
assert_eq!(bindings.document().text("#app"), Some("hello".into()));
```

M11 keeps DOM and fetch behind host capabilities. TypeScript calls named binding
functions that cross the same `InteropValue` boundary introduced in M9. Fetch
allows relative and same-origin URLs and rejects cross-origin URLs before data
enters TSVM.

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

The implementation document's first full standalone pass is now represented in
the repository. Remaining work is deeper browser integration, broader language
coverage, production hardening, and performance engineering.

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

All milestones from the implementation document have a repository-backed
implementation or executable standalone model. See `docs/roadmap.md` for the
next deeper integration passes and `docs/benchmark-results.md` for the first
checked-in benchmark baseline.

See [`docs/roadmap.md`](docs/roadmap.md) and
[`docs/milestones.md`](docs/milestones.md).

## Contributing

Keep the central invariant intact. Do not add a TypeScript-to-JavaScript
execution path, even for demos or tests. New runtime behavior should arrive with
tests first, fixture coverage when possible, and documentation for any boundary
that affects browser security.
