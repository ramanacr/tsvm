# ADR-0005: Runtime Type Modes

## Status

Accepted for planning.

## Decision

TSVM will design for compatible mode and strict runtime types mode. Compatible
mode keeps behavior close to JavaScript while using types for analysis,
diagnostics, metadata, and future optimization. Strict mode treats annotations
as runtime contracts at selected boundaries.

## Consequences

The runtime can start compatible while preserving a path toward stronger type
contracts. External data remains untrusted in both modes.

