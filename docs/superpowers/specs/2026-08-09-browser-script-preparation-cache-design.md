# Browser Script Preparation Cache Design

## Purpose

M16 reduces repeat browser-script startup work without changing TSVM's execution
model. M15 showed that retained verified bytecode is much cheaper than a fresh
compile-and-execute cycle, but the standalone script loader still recompiles a
script every time it is asked to execute one.

M16 introduces a bounded, page-owned preparation cache. The cache retains only
`PreparedModule` values that were built through TSVM's normal parse, semantic,
IR, bytecode, and verifier pipeline. It never stores JavaScript, skips no
security check, and is not a process-wide cross-origin cache.

## Scope

The milestone has four deliverables:

1. A bounded cache in `tsvm-interpreter` for immutable verified prepared
   modules, keyed by exact TypeScript source text.
2. A page-session API in `browser/script-loader` that applies script policy,
   looks up or prepares the module, then executes it with the supplied host.
3. Deterministic cache hit, miss, and eviction observability for tests and
   benchmarks.
4. A browser-style benchmark and published result set that separate source
   preparation from cached script execution.

The existing one-shot APIs remain source-compatible. Existing loader entry
points keep their current behavior by constructing a short-lived page session
for a single call.

## Non-Goals

M16 does not add a JIT, executable-code generation, TypeScript-to-JavaScript
fallback, shared process cache, disk cache, browser network cache, Chromium
source change, Blink hook, timer, event loop, promise, or async fetch feature.

It does not infer cache identity from a URL, an origin, a file modification
time, or an unverified hash. Exact source text is the only cache key in this
standalone milestone, which makes invalidation deterministic and avoids
accidentally reusing bytecode for changed text.

## Architecture

### Verified Module Cache

`PreparedModuleCache` lives in the interpreter crate and owns a bounded set of
private `PreparedModule` values. Its public constructor rejects a zero capacity.
Its `get_or_prepare(source)` method:

1. Looks up the exact source string.
2. Returns an immutable cached prepared module on a hit.
3. Calls `PreparedModule::from_source` on a miss, so compile diagnostics and
   verifier failures remain identical to one-shot execution.
4. Inserts only a successfully verified prepared module.
5. Evicts the oldest inserted source when capacity is exhausted.

The cache uses deterministic FIFO eviction. Hit access does not reorder entries.
FIFO is selected because it has a small auditable implementation, predictable
tests, and no hidden timestamp or global state. A later browser integration can
replace the page-local policy with an explicitly designed browser cache policy.

`PreparedModuleCacheStats` exposes `hits`, `misses`, `evictions`, and
`entries`. Stats count lookup outcomes, not compilation attempts; an invalid
source miss returns its error and is not inserted.

The cache returns a borrow of the immutable `PreparedModule` plus a
`CacheLookup` status. Callers cannot access underlying bytecode or retain a
mutable cache reference while executing a cached module.

### Page Script Session

`PageScriptSession` lives in `browser/script-loader` and owns one
`PreparedModuleCache`. The caller chooses its capacity when constructing a
session. Each `execute_inline_typescript(source, host, policy)` request checks
the current `ScriptPolicy` before cache lookup. If TypeScript is disallowed,
the request fails without recording a cache lookup or compiling source.

After policy approval, the session obtains the prepared module and executes it
with the supplied `HostEnvironment`. Each execution creates the fresh
interpreter and heap state already guaranteed by `PreparedModule`. The session
therefore caches compilation work, never JavaScript runtime state, DOM state,
heap values, console values, or host capabilities.

External script resources continue to resolve through existing loader rules.
M16 applies caching only after a resource has been resolved to concrete source
text; normal resource lookup and script policy behavior remain authoritative.

### Benchmark Evidence

The benchmark runner gains a `cached-page-startup` scenario. It creates one
page script session, performs one cache miss during warm-up, then repeatedly
executes the same source through the session. The expected output remains
`150`; cache stats must show one miss and cache hits for all later lookups.

The existing `page-startup` scenario remains the uncached cold baseline. The
new scenario uses a separate `cached-entry` benchmark mode so CSV output makes
the preparation boundary explicit. The benchmark output adds `cache_hits` and
`cache_misses` columns, with zero values for scenarios that do not use the
session cache.

## Security Rules

- `PreparedModule::from_source` remains the sole cache insertion path.
- Failed compilation or verification is never cached.
- Script policy is evaluated before every session cache lookup and execution.
- The cache is page-session owned; no data crosses page, origin, renderer, or
  process boundaries through M16.
- Host bindings are supplied anew per execution and are never stored by the
  cache.
- All TypeScript execution remains direct TSVM execution; no generated
  JavaScript is introduced.

## Testing

Interpreter tests cover hit/miss accounting, successful-only insertion,
deterministic FIFO eviction, zero-capacity rejection, and execution through a
borrowed cached prepared module.

Script-loader tests cover policy blocking before cache access, repeated inline
script execution with fresh runtime state, host binding behavior after a hit,
and source change producing a cache miss rather than stale reuse.

Benchmark tests cover CSV columns, the cached scenario mode, its expected
console count, and its deterministic cache counters. Existing verifier,
no-generated-JavaScript, same-origin, C ABI, C++20 syntax, workspace lint, and
workspace test gates remain required.

## Published Results

M16 publishes a release-profile benchmark run alongside M13 and M15. The result
record includes the exact command, operating system, Rust version, processor,
sample methodology, raw rows, and cache-counter columns. It compares the
cached-page workload with M16's uncached cold workload on the same machine, but
does not claim a real Chromium or cross-engine result.

## Definition Of Done

M16 is complete when:

1. A zero-capacity-resistant, bounded verified module cache is tested and
   available to the standalone runtime.
2. A page-owned script session applies policy before each cache lookup and
   executes cached verified modules with fresh runtime state.
3. The browser benchmark suite contains both uncached and cached page startup
   scenarios, validates cache counters, and reports the expanded CSV schema.
4. Actual M16 release measurements and methodology are committed to the
   repository.
5. User-facing documentation, milestone status, and roadmap accurately state
   the cache boundary and deferred real Chromium work.
6. All Rust, corpus, security, C ABI, and C++ smoke verification passes.

## Follow-On Work

M17 may use the M16 page-session boundary from a real Chromium/Blink
`text/typescript` dispatch hook after browser policy checks. Production browser
caching, network/cache invalidation, cross-origin partitioning, and disk-backed
artifacts require that real integration and a separate browser security design.
