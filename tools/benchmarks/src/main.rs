#![forbid(unsafe_code)]

use tsvm_benchmarks::{csv_header, run_default_benchmarks};

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100);

    println!("{}", csv_header());
    match run_default_benchmarks(iterations) {
        Ok(results) => {
            for result in results {
                println!("{}", result.csv_row());
            }
        }
        Err(error) => {
            eprintln!("benchmark failed: {error:?}");
            std::process::exit(1);
        }
    }
}
