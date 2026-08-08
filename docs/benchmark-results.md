# Benchmark Results

These results are checked in as the M13 baseline for the standalone TSVM
interpreter. They are useful for regression comparison, not as a final
performance claim.

Run date: 2026-08-08

Command:

```sh
cargo +stable-x86_64-pc-windows-gnu run -p tsvm-benchmarks -- 100
```

Mode: Cargo dev profile on Windows with the GNU Rust target.

```csv
name,iterations,elapsed_micros,console_values
initial-demo,100,17875,100
function-calls,100,9447,100
object-mutation,100,14286,100
```

Interpretation:

- `iterations` is the number of complete compile-and-execute runs per scenario.
- `elapsed_micros` is wall-clock elapsed time for those runs.
- `console_values` is a stable correctness counter; each default scenario logs
  one value per iteration.
