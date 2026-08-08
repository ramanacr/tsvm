# ADR-0002: Chromium Embedding Strategy

## Status

Accepted for planning.

## Decision

TSVM will target a Chromium-based browser and live in renderer processes beside
Blink and V8. Browser-process integration is limited to normal browser policy,
loading, and brokered capabilities.

## Consequences

The project avoids building a rendering engine. TSVM still needs careful script
loading, CSP, DevTools, site-isolation, and binding work before browser use.

