# Browser-Workload Performance

TSVM is interpreter-first. M16 measures the parts of a browser-style script
lifecycle that the standalone runtime can exercise today, without inventing an
event loop or a JavaScript fallback.

Run the default suite:

```sh
cargo run --release -p tsvm-benchmarks -- 1000
```

For the end-to-end narrative demonstration, including a small benchmark
snapshot, run:

```sh
cargo run -p tsvm-demo
```

The benchmark executable emits CSV:

```text
name,mode,iterations,median_elapsed_micros,console_values,cache_hits,cache_misses
```

`iterations` is the number of execution repetitions in each timed sample.
The runner performs one unmeasured warm-up followed by five timed samples and
reports their median wall-clock duration in microseconds. `console_values` is
the validated number of console values from one timed sample; handler dispatch
validates its returned value instead and therefore reports zero console values.
`cache_hits` and `cache_misses` are zero for non-cache scenarios. For
`cached-entry`, they are cumulative session counters: the one warm-up lookup is
the single miss and the five timed samples contribute `5 * iterations` hits.

## Workloads

- `page-startup` / `cold`: compiles TypeScript, verifies bytecode, creates a
  fresh runtime, and executes an initial page script on every iteration.
- `cached-page-startup` / `cached-entry`: owns a one-entry standalone page
  session, prepares the initial page source during the unmeasured warm-up, then
  executes the same verified module with a fresh runtime and host on every
  timed cache hit.
- `prepared-page-entry` / `warm-entry`: compiles and verifies once, then runs
  the retained verified module with fresh runtime state for every iteration.
- `prepared-handler-dispatch` / `warm-handler`: compiles and verifies once,
  then calls a named TypeScript handler through the explicit host-to-TS
  interop boundary.
- `dom-binding-update` / `warm-entry`: exercises retained verified bytecode
  and explicit document read/write bindings against a fresh deterministic host
  fixture for each iteration.
- `same-origin-fetch-update` / `warm-entry`: exercises retained verified
  bytecode, same-origin text fetch, and a document update through explicit
  host bindings.

Every iteration checks its expected console value, handler return value, or
host-visible document text before it contributes to a timing sample. A failed
expectation makes the executable exit nonzero instead of emitting a plausible
but invalid timing result.

## Preparation Cache Boundary

`PageScriptSession` owns a bounded `PreparedModuleCache`. Entries are keyed by
the exact TypeScript source text and are inserted only after TSVM's normal
compile-and-verify pipeline succeeds. The cache uses deterministic FIFO
eviction: hits do not reorder an entry. It contains immutable verified modules,
not JavaScript, bytecode access, heaps, console output, DOM state, fetch
responses, or host capabilities.

The session checks `ScriptPolicy` before every lookup. A denied request neither
prepares source nor changes cache counters. Every allowed execution receives
the caller's current `HostEnvironment` and constructs a fresh interpreter and
heap. External scripts continue through resource resolution and local module
graph bundling before their concrete bundled source can be cached.

The checked-in measurements are in
[`benchmark-results.md`](benchmark-results.md). Each result must record the
command, operating system, Rust toolchain, Cargo profile, processor, sample
count, and raw CSV rows. Results from different machines are useful trend
evidence, not directly comparable rankings. CI validates benchmark correctness
and output shape; it intentionally does not enforce a wall-clock threshold on
shared workers.

## Interpretation

M16 is evidence for the standalone TSVM lifecycle only. It does not measure a
real Chromium renderer, Blink script dispatch, a DOM implementation, timers,
asynchronous fetch, or a JavaScript engine. It must not be used to claim that
TSVM beats a browser renderer or every JavaScript engine. The preparation cache
is page-session-local in this model; it is not a browser network cache, disk
cache, cross-origin cache, or production Chromium caching policy.

A future external comparison requires TSVM to run the same workload inside real
Chromium and must disclose the browser revision, TSVM revision, operating
system, CPU, flags, security mode, warm-up/sample methodology, and functional
equivalence.

## JIT Constraints

JIT work remains research only until the browser integration model is mature.
Any future executable-code generation must preserve:

- renderer sandbox isolation,
- W^X memory policy,
- platform code-signing requirements,
- bytecode verifier gating before optimization,
- deterministic fallback to the interpreter, and
- no TypeScript-to-JavaScript execution path.
