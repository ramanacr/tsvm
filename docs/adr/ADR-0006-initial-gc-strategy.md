# ADR-0006: Initial GC Strategy

## Status

Accepted and implemented for the standalone runtime.

## Decision

The first managed heap should use a simple tracing collector and avoid moving
objects until DOM wrappers, JS interop handles, and TS references are stable.

The M7 implementation uses a reusable Rust `runtime/heap` crate with explicit
roots, stable non-moving slots, and generation-checked handles. The interpreter
allocates objects and arrays on this heap and treats return values plus
`console.log` arguments as host-boundary roots before final collection.

## Consequences

This favors correctness and boundary safety over early peak performance.
Handle ownership and tracing roots need explicit tests.
