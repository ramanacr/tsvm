# Threat Model

## Assets

- Browser process privileges and brokered operating-system capabilities.
- Renderer isolation boundaries between origins and sites.
- User data reachable through DOM, storage, cookies, network credentials, and
  browser APIs.
- Integrity of the TSVM compiler pipeline and bytecode verifier.
- Availability of the renderer process under malicious TypeScript input.

## Trust Boundaries

Every page, script, module, network response, decoded asset, and renderer
process is untrusted. TSVM must run inside the renderer sandbox. The future
browser process, network service, storage service, and GPU service remain
privileged relative to TypeScript code.

## Primary Threats

- Parser, semantic, bytecode, or interop crashes caused by hostile input.
- Resource exhaustion through deeply nested syntax, large source files,
  recursive types, large bytecode, or long-running interpreter loops.
- Verifier bypass that allows malformed bytecode to execute.
- Capability confusion where TypeScript obtains raw filesystem, socket, or
  browser-process privileges.
- Origin, CSP, or site-isolation bypass through typed APIs.
- Unsafe JS/TS interop conversions, callback ownership bugs, and exception or
  promise conversion mistakes.

## Required Defenses

- Rust for input-heavy runtime components where practical.
- Mandatory bytecode verification before interpreter execution.
- Explicit resource limits for source size, nesting, type recursion,
  compilation time, bytecode size, heap size, and selected interpreter steps.
- Capability-checked wrappers for DOM, fetch, console, timers, and interop.
- Fuzzing for lexer, parser, semantic analyzer, bytecode decoder, verifier,
  module loader, and interop boundary.
- Security regression tests for origin policy, CSP, verifier rejection, and
  crash corpus entries.

