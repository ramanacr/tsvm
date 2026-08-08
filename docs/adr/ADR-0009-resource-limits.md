# ADR-0009: Resource Limits

## Status

Accepted.

## Decision

TSVM must enforce resource limits for source size, AST nesting, parser
recursion, generic instantiation, type recursion, semantic-analysis memory,
compilation time, bytecode size, runtime heap, and selected interpreter steps.

## Consequences

Limits must be configurable for tests and hardened for browser use. Fuzzing and
regression tests should exercise limit failures as successful defensive
behavior.

