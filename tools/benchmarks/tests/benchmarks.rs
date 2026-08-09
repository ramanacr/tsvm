use tsvm_benchmarks::{run_default_benchmarks, BenchmarkMode};

#[test]
fn default_benchmarks_cover_browser_workload_modes() {
    let results = run_default_benchmarks(1).expect("workloads should execute");
    let names = results
        .iter()
        .map(|result| result.name.as_str())
        .collect::<Vec<_>>();
    let modes = results.iter().map(|result| result.mode).collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "page-startup",
            "prepared-page-entry",
            "prepared-handler-dispatch",
            "dom-binding-update",
            "same-origin-fetch-update",
        ]
    );
    assert_eq!(
        modes,
        vec![
            BenchmarkMode::Cold,
            BenchmarkMode::WarmEntry,
            BenchmarkMode::WarmHandler,
            BenchmarkMode::WarmEntry,
            BenchmarkMode::WarmEntry,
        ]
    );
}

#[test]
fn benchmark_runner_validates_each_browser_workload_iteration() {
    let results = run_default_benchmarks(2).expect("workloads should execute");

    assert_eq!(results.len(), 5);
    assert_eq!(
        results
            .iter()
            .map(|result| result.console_values)
            .collect::<Vec<_>>(),
        vec![2, 2, 0, 2, 2]
    );
    assert!(results.iter().all(|result| result.iterations == 2));
}
