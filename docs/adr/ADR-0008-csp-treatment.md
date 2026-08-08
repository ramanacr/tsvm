# ADR-0008: CSP Treatment For TypeScript

## Status

Accepted.

## Decision

`script-src` controls JavaScript and TypeScript execution in the initial
browser integration.

## Consequences

TypeScript cannot become a weaker script loading path. A future
`typescript-src` directive requires standards justification and compatibility
analysis.

