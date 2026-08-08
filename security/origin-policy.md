# Origin Policy

TypeScript must obey the same-origin policy exactly as JavaScript does.

Typed APIs such as `fetch<T>()`, `postMessage<T>()`, typed DOM accessors, and
strict runtime type boundaries must not upgrade untrusted data into trusted data
or bypass origin checks.

Current standalone binding coverage includes same-origin text fetch acceptance
and cross-origin fetch rejection in `web-bindings/dom-fetch`.

Future origin tests should cover:

- credential handling,
- module graph loading,
- `postMessage` validation,
- DOM access across frame boundaries,
- exception paths that cross JS/TS interop.
