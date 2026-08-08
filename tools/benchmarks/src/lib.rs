#![forbid(unsafe_code)]

use std::time::{Duration, Instant};

use tsvm_interpreter::execute_source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub elapsed: Duration,
    pub console_values: usize,
}

pub fn run_default_benchmarks(iterations: usize) -> Vec<BenchmarkResult> {
    scenarios()
        .into_iter()
        .map(|scenario| run_benchmark(scenario.name, scenario.source, iterations))
        .collect()
}

pub fn run_benchmark(name: &str, source: &str, iterations: usize) -> BenchmarkResult {
    let started = Instant::now();
    let mut console_values = 0;
    for _ in 0..iterations {
        let output = execute_source(source).expect("benchmark source should execute");
        console_values += output.console.len();
    }
    BenchmarkResult {
        name: name.into(),
        iterations,
        elapsed: started.elapsed(),
        console_values,
    }
}

pub fn scenarios() -> Vec<BenchmarkScenario> {
    vec![
        BenchmarkScenario {
            name: "initial-demo",
            source: r#"
interface Account {
  id: number;
  balance: number;
}

function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
"#,
        },
        BenchmarkScenario {
            name: "function-calls",
            source: r#"
function add(a: number, b: number): number {
  return a + b;
}

console.log(add(add(10, 20), add(5, 7)));
"#,
        },
        BenchmarkScenario {
            name: "object-mutation",
            source: r#"
const account = {
  id: 1,
  balance: 100
};

account.balance += 25;
account.balance += 25;
console.log(account.balance);
"#,
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkScenario {
    pub name: &'static str,
    pub source: &'static str,
}
