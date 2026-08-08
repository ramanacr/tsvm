# ADR-0007: JS/TS Interop Value Model

## Status

Accepted and implemented for the standalone host boundary.

## Decision

JS/TS interop is a security and correctness boundary. Values crossing the
boundary require type checks, shape checks, provenance checks, exception
conversion, promise conversion, and explicit handle ownership.

The M9 standalone runtime introduces `runtime/interop` with explicit
`InteropValue` variants and a named `HostEnvironment` function registry.
Interpreter calls into registered host functions convert values through this
boundary and report host failures as interop errors. Host code can also prepare
a verified TS module and call named TS functions with interop arguments.

## Consequences

The first interop boundary supports primitives, objects, arrays, `null`,
`undefined`, named host calls, host-to-TS function calls, and error propagation.
Browser embedding still must account for real V8 values, classes, promises,
exceptions, callbacks, getters, setters, symbols, proxies, typed arrays, and
eventual `ArrayBuffer` support.
