# ADR-0010: JIT Deferral

## Status

Accepted.

## Decision

The first runtime uses an interpreter over verified bytecode. JIT compilation
is deferred until the interpreter, verifier, security model, and benchmark
suite are mature.

M13 adds `tools/benchmarks` as the first interpreter benchmark baseline. JIT
research must use that baseline before proposing executable-code generation.

## Consequences

Early milestones avoid executable memory, W^X policy complexity, code signing
questions, and a larger attack surface. Performance research begins after the
runtime is behaviorally correct.

Any future JIT prototype must preserve W^X, platform code-signing constraints,
renderer sandbox assumptions, verifier gating, deterministic interpreter
fallback, and the central no-TypeScript-to-JavaScript execution invariant.
