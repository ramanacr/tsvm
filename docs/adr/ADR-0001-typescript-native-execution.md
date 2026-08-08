# ADR-0001: TypeScript-Native Execution

## Status

Accepted.

## Decision

TSVM executes TypeScript through a TypeScript-native pipeline:

```text
TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM
```

The runtime will not use TypeScript-to-JavaScript transpilation as an execution
path.

## Consequences

This preserves the project goal of a second first-class scripting runtime. It
also means early milestones must build more infrastructure before browser demos
are possible.

