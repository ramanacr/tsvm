# TypeScript-Native Browser Runtime Implementation Specification

## Purpose

This document defines a repo-ready implementation plan for a browser runtime that executes TypeScript directly through a TypeScript-native virtual machine, instead of converting TypeScript source into JavaScript.

The central invariant is:

```text
TypeScript
  -> Typed AST
  -> Semantic Analysis
  -> Typed IR
  -> Verified Bytecode
  -> TSVM

No TypeScript -> JavaScript execution path.
```

JavaScript remains supported through the browser's normal JavaScript engine. TypeScript becomes a second first-class scripting runtime hosted by the browser.

## Product Definition

The project is not merely a browser that accepts `.ts` files. It is a TypeScript-native execution runtime embedded into a Chromium-based browser as a second scripting engine alongside V8.

Target browser behavior:

```html
<script type="text/typescript" src="/app.ts"></script>
```

The browser loads `app.ts`, parses it as TypeScript, performs semantic analysis, emits verified TS bytecode, and executes that bytecode in the TSVM.

## Non-Goals For V0

- No TypeScript-to-JavaScript transpilation.
- No JIT compiler in the first implementation.
- No Angular runtime support in the first implementation.
- No Node.js APIs in web pages.
- No raw filesystem access from web TypeScript.
- No raw socket access from web TypeScript.
- No unrestricted `eval` or dynamic TypeScript compilation.
- No replacement of JavaScript or V8.
- No custom rendering engine.

## Initial Architecture

```text
Chromium-Based Browser
  |
  |-- Browser Process
  |     |-- navigation
  |     |-- permissions
  |     |-- security policy
  |     |-- downloads
  |     |-- certificates
  |
  |-- Renderer Process
  |     |-- Blink
  |     |-- V8
  |     |-- TSVM
  |     |     |-- lexer
  |     |     |-- parser
  |     |     |-- semantic analyzer
  |     |     |-- typed IR
  |     |     |-- bytecode generator
  |     |     |-- bytecode verifier
  |     |     |-- interpreter
  |     |     |-- heap and GC
  |     |
  |     |-- DOM bindings
  |     |-- Fetch bindings
  |     |-- JS/TS interop boundary
  |
  |-- Network Service
  |-- Storage Service
  |-- GPU Service
```

## Execution Pipeline

```text
.ts source
  |
  v
Lexer
  |
  v
Parser
  |
  v
Typed AST
  |
  v
Semantic Analyzer
  |
  v
Typed IR
  |
  v
Bytecode Generator
  |
  v
Bytecode Verifier
  |
  v
Interpreter
  |
  v
Browser Capability Layer
  |
  v
DOM / Fetch / Console / Timers / Storage
```

The runtime must be able to prove, by tests and architecture, that no generated JavaScript is used in this path.

## Repository Structure

```text
ts-browser-runtime/
  browser/
    chromium/
    script-loader/
    devtools/
  runtime/
    lexer/
    parser/
    ast/
    type-system/
    semantic/
    ir/
    bytecode/
    verifier/
    interpreter/
    heap/
    gc/
    diagnostics/
  web-bindings/
    console/
    dom/
    fetch/
    timers/
    events/
    workers/
  interop/
    js-to-ts/
    ts-to-js/
    shared-values/
  security/
    threat-model.md
    sandbox.md
    csp.md
    origin-policy.md
  tests/
    fixtures/
    lexer/
    parser/
    semantic/
    bytecode/
    verifier/
    interpreter/
    web-bindings/
    interop/
    security/
    fuzz/
  tools/
    disassembler/
    bytecode-dump/
    corpus-runner/
  docs/
    adr/
    roadmap.md
    milestones.md
```

## Implementation Language

Recommended split:

- Rust for lexer, parser, AST, semantic analysis, bytecode verification, module loading, runtime metadata, and fuzzable components.
- C++ for Chromium integration and Blink/V8 boundary work.
- A stable C ABI or CXX bridge between Chromium-side C++ and Rust runtime components.

Rust does not remove all security risk, but it reduces memory corruption risk in the most input-heavy parts of the system.

## TypeScript Support Scope

### V0.1 Supported Syntax

- `let`, `const`, and `var`
- primitive types: `number`, `string`, `boolean`, `null`, `undefined`
- object literals
- arrays
- functions
- arrow functions
- interfaces
- type aliases for simple structural types
- classes without decorators
- imports and exports for local modules
- `if`, `else`, `switch`
- `for`, `while`, `do`
- exceptions
- basic async functions can be deferred until V0.2

### Deferred Syntax

- decorators
- namespaces
- complex conditional types
- mapped types
- `infer`
- advanced overload resolution
- JSX
- Angular templates
- runtime TypeScript `eval`
- JIT-specific syntax metadata

## Runtime Semantics

Two execution modes should be designed, though only one may ship initially.

### Compatible Mode

TypeScript annotations guide analysis, diagnostics, metadata, and optimization, but runtime behavior remains close to JavaScript semantics.

### Strict Runtime Types Mode

Type annotations become runtime contracts. Boundary values are checked, and violations raise runtime type errors.

Example:

```ts
function add(a: number, b: number): number {
  return a + b;
}

add("1" as any, 2);
```

In strict mode, this fails before executing `a + b`.

## Typed IR

The runtime should lower TypeScript into a typed intermediate representation.

Example source:

```ts
function square(x: number): number {
  return x * x;
}
```

Example IR:

```text
function square(number) -> number
block0:
  %0:number = load_arg 0
  %1:number = number_mul %0, %0
  return %1
```

The IR should preserve enough type metadata for diagnostics, verifier checks, and future optimization.

## Bytecode

The first runtime should use an interpreter over verified bytecode.

Example bytecode:

```text
load_const 10
store_local price
load_local price
load_const 2
number_mul
call console.log 1
return
```

Required bytecode properties:

- deterministic encoding
- explicit operand widths
- stable version header
- constant pool
- function table
- exception table
- source map references
- verifier-visible type metadata

## Bytecode Verifier

The verifier is mandatory. It must treat compiler output as untrusted.

Verifier checks:

- valid opcode
- valid operand count
- valid register index
- valid local index
- valid constant pool reference
- valid function reference
- valid jump target
- valid control-flow graph
- valid stack/register state at block boundaries
- valid exception table entries
- valid type tags
- no malformed function boundaries
- no invalid capability access

The interpreter should only execute verified bytecode.

## Memory And GC

The TSVM needs its own managed heap or a carefully integrated heap strategy with the host runtime.

Initial recommendation:

- Start with a simple tracing garbage collector.
- Avoid moving GC until object references, interop handles, and DOM wrappers are stable.
- Use explicit handles for JS/TS/DOM boundary values.
- Keep browser objects behind capability-checked wrappers.

Object representation options:

- boxed dynamic values for early compatibility
- typed records for strict-mode objects
- tagged values for primitives
- stable handles for DOM and JS interop objects

## JS And TS Interoperability

Interop is a security and correctness boundary.

```text
JavaScript
  |
  v
Interop Boundary
  |-- type checks
  |-- shape checks
  |-- provenance checks
  |-- exception conversion
  |-- promise conversion
  |-- handle ownership
  v
TSVM
```

The reverse direction needs the same rigor.

Interop must account for:

- primitives
- objects
- arrays
- functions
- classes
- promises
- exceptions
- callbacks
- getters and setters
- symbols
- proxies
- typed arrays
- `ArrayBuffer`
- `SharedArrayBuffer`, deferred until later

## Browser Bindings

Initial bindings:

- `console`
- `document`
- `window`
- DOM query APIs
- DOM events
- timers
- `fetch`
- basic `Response` and `Request`
- basic module loading

Deferred bindings:

- WebSocket
- Worker
- IndexedDB
- Canvas
- WebGL
- WebGPU
- WebRTC
- advanced streams

All bindings must route through the same browser security model as JavaScript.

## Security Model

Every page, script, module, network response, decoded asset, and renderer process is untrusted.

The TSVM must live inside the renderer sandbox. It must not run in the privileged browser process.

```text
Untrusted TypeScript
  |
  v
Parser / Semantic Analyzer
  |
  v
Verified Bytecode
  |
  v
Interpreter
  |
  v
Capability Layer
  |
  v
Browser IPC / Broker
  |
  v
Privileged Resources
```

TypeScript code must never directly possess OS capabilities.

### Same-Origin Policy

TypeScript must obey the same-origin policy exactly as JavaScript does.

Typed APIs such as `fetch<T>()`, `postMessage<T>()`, or typed DOM access must not bypass origin checks.

### Site Isolation

The TSVM must participate in the browser's site isolation model.

```text
bank.com -> Renderer A -> TSVM A
evil.com -> Renderer B -> TSVM B
```

A TSVM compromise in one site must not expose another site.

### CSP

Content Security Policy must apply to TypeScript execution.

Initial rule:

```text
script-src controls JavaScript and TypeScript.
```

A future `typescript-src` directive should only be considered if standards work justifies it.

### Runtime Type Trust

Static TypeScript types do not make external data trusted.

```ts
const user: User = await response.json();
```

The runtime must treat the response as untrusted unless it passes explicit validation.

Strict mode may support:

```ts
const user = User.validate(await response.json());
```

or an equivalent runtime schema mechanism.

## Resource Limits

The runtime must defend against resource exhaustion.

Required budgets:

- maximum source size
- maximum AST nesting
- maximum parser recursion
- maximum generic instantiation count
- maximum type recursion depth
- maximum semantic-analysis memory
- maximum compilation time
- maximum bytecode size
- maximum runtime heap
- maximum interpreter step budget for selected contexts

These limits should be configurable for tests and hardened for browser use.

## Testing Strategy

Test layers:

- lexer golden tests
- parser fixture tests
- AST snapshot tests
- semantic analyzer tests
- type resolution tests
- IR generation tests
- bytecode encoding tests
- verifier acceptance and rejection tests
- interpreter behavior tests
- module graph tests
- DOM binding tests
- fetch binding tests
- JS/TS interop tests
- security policy tests
- origin tests
- CSP tests
- fuzz tests
- crash regression tests

The project must maintain a corpus of valid and invalid TypeScript programs.

## Fuzzing

Fuzz targets:

- lexer
- parser
- semantic analyzer
- bytecode decoder
- bytecode verifier
- module loader
- interop boundary

Fuzz invariants:

- no memory corruption
- no panics in release mode
- no unbounded recursion
- no verifier bypass
- no interpreter execution of invalid bytecode
- no sandbox escape path

## CI/CD

Minimum CI jobs:

- formatting
- linting
- Rust tests
- C++ tests
- fixture tests
- bytecode verifier tests
- browser smoke tests
- fuzz smoke run
- sanitizer build
- dependency audit
- documentation checks

Recommended sanitizer coverage:

- AddressSanitizer
- UndefinedBehaviorSanitizer
- ThreadSanitizer where practical
- MemorySanitizer where practical

## Milestones

### M0: Engineering Baseline

Acceptance criteria:

- repository scaffolded
- build system works
- CI runs
- formatting and linting enforced
- ADR directory created
- threat model drafted

### M1: Lexer

Acceptance criteria:

- tokenizes V0.1 TypeScript subset
- reports source spans
- handles comments and whitespace
- has golden tests
- has fuzz target

### M2: Parser

Acceptance criteria:

- parses V0.1 syntax
- produces AST with spans
- recovers from common syntax errors
- has fixture coverage

### M3: Semantic Analyzer

Acceptance criteria:

- builds symbols and scopes
- resolves simple structural types
- validates function signatures
- validates local variable usage
- reports clear diagnostics

### M4: Typed IR

Acceptance criteria:

- lowers basic programs into typed IR
- preserves source references
- supports control flow
- supports function calls

### M5: Bytecode And Verifier

Acceptance criteria:

- emits deterministic bytecode
- decodes bytecode safely
- rejects malformed bytecode
- rejects invalid control flow
- rejects invalid type states

### M6: Interpreter

Acceptance criteria:

- executes verified bytecode
- supports primitives, objects, arrays, functions, and classes
- supports exceptions
- passes behavioral fixtures

### M7: Heap And GC

Acceptance criteria:

- managed allocation works
- unreachable objects are collected
- handles cross-boundary references safely
- has stress tests

### M8: Modules

Acceptance criteria:

- supports local module imports and exports
- detects cycles
- resolves module graph deterministically
- reports module diagnostics

### M9: JS Interop

Acceptance criteria:

- JS can call TS functions
- TS can call JS functions
- values cross boundary safely
- exceptions and promises are converted correctly

### M10: Minimal Browser Embed

Acceptance criteria:

- Chromium-based shell recognizes `text/typescript`
- loads `.ts` scripts
- executes through TSVM
- `console.log` works
- no generated JavaScript is used

### M11: DOM And Fetch

Acceptance criteria:

- TS can query and mutate DOM
- TS can handle events
- TS can call `fetch`
- same-origin and CSP rules apply

### M12: Security Hardening

Acceptance criteria:

- sandbox assumptions documented
- origin tests pass
- CSP tests pass
- fuzzing integrated
- verifier bypass tests pass
- crash corpus maintained

### M13: Performance And JIT Research

Acceptance criteria:

- interpreter benchmark suite exists
- hot-path profiling exists
- JIT design ADR drafted
- W^X and code signing implications documented

## Initial Demo Program

The first standalone runtime should execute:

```ts
interface Account {
  id: number;
  balance: number;
}

function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
```

Expected output:

```text
150
```

Required proof:

```text
.ts source
  -> TS parser
  -> Typed AST
  -> TS bytecode
  -> TS interpreter
  -> 150
```

No generated JavaScript may appear in this path.

## ADRs To Create

- ADR-0001: TypeScript-native execution rather than transpilation
- ADR-0002: Chromium embedding strategy
- ADR-0003: Rust/C++ boundary
- ADR-0004: Bytecode verifier as mandatory execution gate
- ADR-0005: Compatible mode versus strict runtime types
- ADR-0006: Initial GC strategy
- ADR-0007: JS/TS interop value model
- ADR-0008: CSP treatment for TypeScript
- ADR-0009: Resource limits and denial-of-service protection
- ADR-0010: JIT deferral

## Codex Bootstrap Prompt

Use this prompt when starting the repository:

```text
Create a new repository for a TypeScript-native browser runtime.

The central invariant is:

TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM.

There must be no TypeScript -> JavaScript execution path.

Start with milestone M0 and M1 only:

1. Create the repository structure.
2. Add initial docs and ADRs.
3. Implement a lexer for the V0.1 TypeScript subset.
4. Add golden lexer tests.
5. Add a small fuzz target if practical.
6. Add CI for formatting, linting, and tests.

Do not implement Chromium integration yet.
Do not implement a parser yet unless the lexer milestone is complete.
Keep the runtime independent from the browser until the standalone execution pipeline is proven.
```

## Recommended Starting Sequence

```text
GitHub Repository
  -> M0 Engineering Baseline
  -> Lexer
  -> Parser
  -> Semantic Analyzer
  -> Typed IR
  -> Bytecode
  -> Bytecode Verifier
  -> Interpreter
  -> Heap / GC
  -> Modules
  -> JS Interop
  -> Minimal Browser Embed
  -> DOM / Fetch
  -> Full Chromium Integration
  -> Performance / JIT Research
```

## Final Acceptance Criteria

The project reaches its first meaningful success when:

- A `.ts` file is loaded by the browser.
- The TypeScript source is parsed directly.
- Typed IR and verified bytecode are produced.
- The TSVM interpreter executes that bytecode.
- The script can call `console.log`.
- The script can perform a minimal DOM mutation.
- The script can call `fetch` subject to normal browser security rules.
- Tests demonstrate that no JavaScript is generated or executed for the TypeScript path.

