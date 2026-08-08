# ADR-0007: JS/TS Interop Value Model

## Status

Accepted for planning.

## Decision

JS/TS interop is a security and correctness boundary. Values crossing the
boundary require type checks, shape checks, provenance checks, exception
conversion, promise conversion, and explicit handle ownership.

## Consequences

Interop is deferred until the standalone runtime is proven. When implemented,
it must account for primitives, objects, arrays, functions, classes, promises,
exceptions, callbacks, getters, setters, symbols, proxies, typed arrays, and
eventual `ArrayBuffer` support.

