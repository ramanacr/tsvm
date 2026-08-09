# ADR-0003: Rust/C++ Boundary

## Status

Accepted. M14 established the one-shot ABI; M17 extends it with opaque
page-session ownership.

## Decision

Input-heavy runtime components are implemented in Rust. Chromium integration is
expected to use C++. The boundary uses a stable C ABI with a narrow C++20
adapter and explicit ownership rules. Rust owns every result and page-session
allocation, validates raw length-delimited input, and contains panics before
they can unwind into C++. C++ owns session lifetime through move-only RAII and
copies borrowed result bytes before releasing their Rust owner.

## Consequences

Rust reduces memory-corruption risk in lexer, parser, semantic, bytecode, and
verifier code. The boundary must still be threat-modeled and fuzzed. The M17
session is intentionally source-only: browser script policy, CSP, resource
loading, origin/site-isolation checks, cache partitioning, invalidation, and
renderer lifecycle decisions stay on the future browser side of the boundary.

