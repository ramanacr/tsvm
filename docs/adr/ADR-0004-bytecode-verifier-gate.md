# ADR-0004: Bytecode Verifier Gate

## Status

Accepted.

## Decision

The interpreter may only execute bytecode that has passed a mandatory verifier.
Compiler output is treated as untrusted.

## Consequences

The verifier becomes part of the security boundary. Tests must include accepted
and rejected bytecode fixtures, malformed control flow, invalid type states,
bad operands, and invalid capability access.

