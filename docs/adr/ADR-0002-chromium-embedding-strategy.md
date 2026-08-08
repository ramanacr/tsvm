# ADR-0002: Chromium Embedding Strategy

## Status

Accepted for planning; standalone script-loader model implemented.

## Decision

TSVM will target a Chromium-based browser and live in renderer processes beside
Blink and V8. Browser-process integration is limited to normal browser policy,
loading, and brokered capabilities.

M10 adds `browser/script-loader` as a standalone Rust model of the future
Chromium script hook. It recognizes `text/typescript`, resolves local `.ts`
resources, executes them through the verified TSVM path, leaves JavaScript
scripts to the normal browser engine, and records that no JavaScript is
generated for TypeScript execution.

## Consequences

The project avoids building a rendering engine. TSVM still needs careful script
loading, CSP, DevTools, site-isolation, and binding work before browser use.
The standalone loader is executable contract coverage, not a replacement for
the eventual C++ renderer integration.
