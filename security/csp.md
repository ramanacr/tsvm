# Content Security Policy

Initial rule: `script-src` controls both JavaScript and TypeScript execution.

TSVM must not create a weaker TypeScript-specific script policy. A future
`typescript-src` directive should only be considered after standards work and
must not silently bypass existing `script-src` expectations.

Policy tests for future milestones must prove that blocked script sources are
blocked for TypeScript as well as JavaScript.

