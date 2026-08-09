#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use tsvm_interop::InteropValue;
use tsvm_interpreter::{
    execute_source, execute_source_with_host, ExecuteError, ExecutionOutput, PreparedModule, Value,
};
use tsvm_web_bindings::{BrowserBindings, Document, FetchService};

pub const MEASURED_SAMPLES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkMode {
    Cold,
    WarmEntry,
    WarmHandler,
}

impl BenchmarkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::WarmEntry => "warm-entry",
            Self::WarmHandler => "warm-handler",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkResult {
    pub name: String,
    pub mode: BenchmarkMode,
    pub iterations: usize,
    pub median_elapsed: Duration,
    pub console_values: usize,
}

impl BenchmarkResult {
    pub fn csv_row(&self) -> String {
        format!(
            "{},{},{},{},{}",
            self.name,
            self.mode.as_str(),
            self.iterations,
            self.median_elapsed.as_micros(),
            self.console_values
        )
    }
}

#[derive(Debug)]
pub enum BenchmarkError {
    InvalidIterations,
    Execute(ExecuteError),
    Expectation {
        scenario: &'static str,
        expected: String,
        actual: String,
    },
}

impl From<ExecuteError> for BenchmarkError {
    fn from(error: ExecuteError) -> Self {
        Self::Execute(error)
    }
}

pub fn csv_header() -> &'static str {
    "name,mode,iterations,median_elapsed_micros,console_values"
}

pub fn run_default_benchmarks(iterations: usize) -> Result<Vec<BenchmarkResult>, BenchmarkError> {
    if iterations == 0 {
        return Err(BenchmarkError::InvalidIterations);
    }

    scenarios()
        .into_iter()
        .map(|scenario| run_benchmark(&scenario, iterations))
        .collect()
}

fn run_benchmark(
    scenario: &BenchmarkScenario,
    iterations: usize,
) -> Result<BenchmarkResult, BenchmarkError> {
    let prepared = match scenario.mode {
        BenchmarkMode::Cold => None,
        BenchmarkMode::WarmEntry | BenchmarkMode::WarmHandler => {
            Some(PreparedModule::from_source(scenario.source)?)
        }
    };

    run_iteration(scenario, prepared.as_ref())?;

    let mut samples = Vec::with_capacity(MEASURED_SAMPLES);
    let mut console_values = None;
    for _ in 0..MEASURED_SAMPLES {
        let started = Instant::now();
        let mut sample_console_values = 0;
        for _ in 0..iterations {
            sample_console_values += run_iteration(scenario, prepared.as_ref())?;
        }
        samples.push(started.elapsed());

        if let Some(expected) = console_values {
            if expected != sample_console_values {
                return Err(expectation_error(
                    scenario,
                    format!("{expected} console values per sample"),
                    format!("{sample_console_values} console values per sample"),
                ));
            }
        } else {
            console_values = Some(sample_console_values);
        }
    }

    Ok(BenchmarkResult {
        name: scenario.name.into(),
        mode: scenario.mode,
        iterations,
        median_elapsed: median_duration(samples),
        console_values: console_values.expect("benchmark sample count is nonzero"),
    })
}

fn run_iteration(
    scenario: &BenchmarkScenario,
    prepared: Option<&PreparedModule>,
) -> Result<usize, BenchmarkError> {
    match scenario.mode {
        BenchmarkMode::WarmHandler => {
            let prepared = prepared.expect("warm handler must be prepared");
            let value = prepared.call_function(
                "handleClick",
                &[InteropValue::Number(41.0)],
                &tsvm_interop::HostEnvironment::new(),
            )?;
            if value != InteropValue::Number(42.0) {
                return Err(expectation_error(
                    scenario,
                    "handler return value 42".into(),
                    format!("handler return value {value:?}"),
                ));
            }
            Ok(0)
        }
        BenchmarkMode::Cold | BenchmarkMode::WarmEntry => {
            let bindings = match scenario.host {
                HostFixture::Empty => None,
                HostFixture::Browser => Some(browser_bindings()),
            };
            let host = bindings
                .as_ref()
                .map(BrowserBindings::host_environment)
                .unwrap_or_else(tsvm_interop::HostEnvironment::new);
            let output = match (scenario.mode, prepared) {
                (BenchmarkMode::Cold, _) if matches!(scenario.host, HostFixture::Empty) => {
                    execute_source(scenario.source)?
                }
                (BenchmarkMode::Cold, _) => execute_source_with_host(scenario.source, &host)?,
                (BenchmarkMode::WarmEntry, Some(prepared)) => prepared.execute_with_host(&host)?,
                (BenchmarkMode::WarmEntry, None) => unreachable!("warm entry must be prepared"),
                (BenchmarkMode::WarmHandler, _) => unreachable!("handler branch returns above"),
            };

            validate_entry_output(scenario, &output, bindings.as_ref())
        }
    }
}

fn validate_entry_output(
    scenario: &BenchmarkScenario,
    output: &ExecutionOutput,
    bindings: Option<&BrowserBindings>,
) -> Result<usize, BenchmarkError> {
    let expected_console = scenario
        .expectation
        .console
        .as_ref()
        .expect("entry expects console output");
    if output.console != vec![expected_console.clone()] {
        return Err(expectation_error(
            scenario,
            format!("console [{expected_console:?}]"),
            format!("console {:?}", output.console),
        ));
    }

    if let Some(expected_text) = scenario.expectation.document_text {
        let actual_text = bindings
            .expect("document expectation requires browser bindings")
            .document()
            .text("#app");
        if actual_text.as_deref() != Some(expected_text) {
            return Err(expectation_error(
                scenario,
                format!("document #app text {expected_text:?}"),
                format!("document #app text {actual_text:?}"),
            ));
        }
    }

    Ok(output.console.len())
}

fn expectation_error(
    scenario: &BenchmarkScenario,
    expected: String,
    actual: String,
) -> BenchmarkError {
    BenchmarkError::Expectation {
        scenario: scenario.name,
        expected,
        actual,
    }
}

fn median_duration(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn scenarios() -> Vec<BenchmarkScenario> {
    vec![
        BenchmarkScenario {
            name: "page-startup",
            mode: BenchmarkMode::Cold,
            host: HostFixture::Empty,
            expectation: ScenarioExpectation {
                console: Some(Value::Number(150.0)),
                document_text: None,
            },
            source: PAGE_ENTRY_SOURCE,
        },
        BenchmarkScenario {
            name: "prepared-page-entry",
            mode: BenchmarkMode::WarmEntry,
            host: HostFixture::Empty,
            expectation: ScenarioExpectation {
                console: Some(Value::Number(150.0)),
                document_text: None,
            },
            source: PAGE_ENTRY_SOURCE,
        },
        BenchmarkScenario {
            name: "prepared-handler-dispatch",
            mode: BenchmarkMode::WarmHandler,
            host: HostFixture::Empty,
            expectation: ScenarioExpectation {
                console: None,
                document_text: None,
            },
            source: HANDLER_SOURCE,
        },
        BenchmarkScenario {
            name: "dom-binding-update",
            mode: BenchmarkMode::WarmEntry,
            host: HostFixture::Browser,
            expectation: ScenarioExpectation {
                console: Some(Value::String("updated by TSVM".into())),
                document_text: Some("updated by TSVM"),
            },
            source: DOM_BINDING_SOURCE,
        },
        BenchmarkScenario {
            name: "same-origin-fetch-update",
            mode: BenchmarkMode::WarmEntry,
            host: HostFixture::Browser,
            expectation: ScenarioExpectation {
                console: Some(Value::String("hello from TSVM".into())),
                document_text: Some("hello from TSVM"),
            },
            source: FETCH_UPDATE_SOURCE,
        },
    ]
}

const PAGE_ENTRY_SOURCE: &str = r#"
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
"#;

const HANDLER_SOURCE: &str = r#"
function handleClick(current: number): number {
  const state = { clicks: current };
  state.clicks += 1;
  return state.clicks;
}
"#;

const DOM_BINDING_SOURCE: &str = r##"
function domText(selector: string): string {
  return "";
}

function domSetText(selector: string, text: string): undefined {
  return undefined;
}

domSetText("#app", "updated by TSVM");
console.log(domText("#app"));
"##;

const FETCH_UPDATE_SOURCE: &str = r##"
function domText(selector: string): string {
  return "";
}

function domSetText(selector: string, text: string): undefined {
  return undefined;
}

function fetchText(url: string): string {
  return "";
}

domSetText("#app", fetchText("/message.txt"));
console.log(domText("#app"));
"##;

#[derive(Debug, Clone)]
struct BenchmarkScenario {
    name: &'static str,
    mode: BenchmarkMode,
    host: HostFixture,
    expectation: ScenarioExpectation,
    source: &'static str,
}

#[derive(Debug, Clone)]
struct ScenarioExpectation {
    console: Option<Value>,
    document_text: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
enum HostFixture {
    Empty,
    Browser,
}

fn browser_bindings() -> BrowserBindings {
    BrowserBindings::new(
        Document::from_text_nodes([("#app", "")]),
        FetchService::new(
            "https://example.test",
            BTreeMap::from([("/message.txt".into(), "hello from TSVM".into())]),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_duration_returns_the_middle_sorted_sample() {
        let median = median_duration(vec![
            Duration::from_micros(90),
            Duration::from_micros(10),
            Duration::from_micros(50),
            Duration::from_micros(30),
            Duration::from_micros(70),
        ]);

        assert_eq!(median, Duration::from_micros(50));
    }

    #[test]
    fn csv_rows_include_the_documented_header_and_mode() {
        let result = BenchmarkResult {
            name: "page-startup".into(),
            mode: BenchmarkMode::Cold,
            iterations: 1,
            median_elapsed: Duration::from_micros(42),
            console_values: 1,
        };

        assert_eq!(
            csv_header(),
            "name,mode,iterations,median_elapsed_micros,console_values"
        );
        assert_eq!(result.csv_row(), "page-startup,cold,1,42,1");
    }
}
