#![forbid(unsafe_code)]

use tsvm_benchmarks::run_default_benchmarks;

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100);

    for result in run_default_benchmarks(iterations) {
        println!(
            "{},{},{},{}",
            result.name,
            result.iterations,
            result.elapsed.as_micros(),
            result.console_values
        );
    }
}
