# Benchmark Results

These checked-in measurements are reproducible standalone TSVM evidence, not a
browser-engine ranking. The command, toolchain, profile, processor, and modes
matter when interpreting them.

## M13 Historical Baseline

Run date: 2026-08-08

Command:

```sh
cargo +stable-x86_64-pc-windows-gnu run -p tsvm-benchmarks -- 100
```

Environment: Windows, Cargo development profile, GNU Rust target.

```csv
name,iterations,elapsed_micros,console_values
initial-demo,100,17875,100
function-calls,100,9447,100
object-mutation,100,14286,100
```

M13 measured a complete source compile-and-execute cycle for each iteration.
It remains historical context; its three scenarios and development profile are
not directly comparable to M15's lifecycle-specific release results.

## M15 Browser-Workload Baseline

Run date: 2026-08-09

Command:

```sh
cargo +stable-x86_64-pc-windows-gnu run --release -p tsvm-benchmarks -- 1000
```

Environment:

- Operating system: Microsoft Windows NT 10.0.26200.0.
- Processor: AMD Ryzen 7 7840HS w/ Radeon 780M Graphics.
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
- Cargo profile: `release`.
- Samples: one unmeasured warm-up plus five timed samples per scenario; median
  elapsed time is reported below.

```csv
name,mode,iterations,median_elapsed_micros,console_values
page-startup,cold,1000,34881,1000
prepared-page-entry,warm-entry,1000,2022,1000
prepared-handler-dispatch,warm-handler,1000,760,0
dom-binding-update,warm-entry,1000,1934,1000
same-origin-fetch-update,warm-entry,1000,2105,1000
```

`cold` includes compilation, verification, and execution for every iteration.
`warm-entry` and `warm-handler` compile and verify once before the warm-up,
then run retained verified bytecode with fresh runtime state. The handler row
checks its return value rather than logging, so `console_values` is zero.

The M15 rows demonstrate the cost separation inside TSVM's standalone runtime.
They do not measure real Chromium integration or establish cross-engine
performance claims.
