# ADR-0006: Initial GC Strategy

## Status

Accepted for planning.

## Decision

The first managed heap should use a simple tracing collector and avoid moving
objects until DOM wrappers, JS interop handles, and TS references are stable.

## Consequences

This favors correctness and boundary safety over early peak performance.
Handle ownership and tracing roots need explicit tests.

