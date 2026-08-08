# Performance And JIT Research

TSVM remains interpreter-first. The M13 benchmark runner gives the project a
repeatable baseline before any optimizer or JIT work begins.

Run:

```sh
cargo run -p tsvm-benchmarks -- 100
```

For a more narrative end-to-end runtime demonstration, run:

```sh
cargo run -p tsvm-demo
```

Output is CSV:

```text
name,iterations,elapsed_micros,console_values
```

The first checked-in local baseline is in
[`benchmark-results.md`](benchmark-results.md).

The default scenarios cover:

- initial demo execution,
- nested function calls,
- object mutation and member reads.

## JIT Constraints

JIT work is research only until the browser integration model is mature. Any
future executable-code generation must preserve:

- renderer sandbox isolation,
- W^X memory policy,
- platform code-signing requirements,
- bytecode verifier gating before optimization,
- deterministic fallback to the interpreter,
- no TypeScript-to-JavaScript execution path.
