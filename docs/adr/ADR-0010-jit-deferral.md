# ADR-0010: JIT Deferral

## Status

Accepted.

## Decision

The first runtime uses an interpreter over verified bytecode. JIT compilation
is deferred until the interpreter, verifier, security model, and benchmark
suite are mature.

## Consequences

Early milestones avoid executable memory, W^X policy complexity, code signing
questions, and a larger attack surface. Performance research begins after the
runtime is behaviorally correct.

