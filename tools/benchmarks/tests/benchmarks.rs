use tsvm_benchmarks::{run_default_benchmarks, scenarios};

#[test]
fn default_benchmarks_cover_core_runtime_paths() {
    let names = scenarios()
        .into_iter()
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();

    assert!(names.contains(&"initial-demo"));
    assert!(names.contains(&"function-calls"));
    assert!(names.contains(&"object-mutation"));
}

#[test]
fn benchmark_runner_reports_iterations_and_console_counts() {
    let results = run_default_benchmarks(2);

    assert_eq!(results.len(), 3);
    for result in results {
        assert_eq!(result.iterations, 2);
        assert_eq!(result.console_values, 2);
    }
}
