# Sandbox Model

TSVM must execute in Chromium renderer processes, never in the privileged
browser process.

```text
Untrusted TypeScript
  -> TSVM compiler pipeline
  -> verified bytecode
  -> interpreter
  -> capability layer
  -> browser IPC or renderer-local browser objects
```

TypeScript code must not receive raw filesystem access, raw sockets, browser
process pointers, unrestricted `eval`, or unrestricted dynamic compilation.

Renderer compromise remains in scope for browser defense in depth. TSVM should
reduce its own attack surface with memory-safe components, verifier gates,
resource budgets, fuzzing, and narrow interop handles.

