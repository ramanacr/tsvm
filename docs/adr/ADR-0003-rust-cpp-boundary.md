# ADR-0003: Rust/C++ Boundary

## Status

Accepted for planning.

## Decision

Input-heavy runtime components are implemented in Rust. Chromium integration is
expected to use C++. The boundary should be a stable C ABI or a narrow CXX
bridge with explicit ownership rules.

## Consequences

Rust reduces memory-corruption risk in lexer, parser, semantic, bytecode, and
verifier code. The boundary must still be threat-modeled and fuzzed.

