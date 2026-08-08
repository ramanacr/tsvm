# Content Security Policy

Initial rule: `script-src` controls both JavaScript and TypeScript execution.

TSVM must not create a weaker TypeScript-specific script policy. A future
`typescript-src` directive should only be considered after standards work and
must not silently bypass existing `script-src` expectations.

The standalone M12 script policy test proves that the TypeScript script loader
can block `text/typescript` execution before compilation. Browser integration
must wire that policy decision to Chromium's real `script-src` enforcement.
