use tsvm_demo::render_demo;

#[test]
fn demo_output_shows_the_runtime_in_action() {
    let output = render_demo();

    assert!(output.contains("TSVM Demo"));
    assert!(output.contains(
        "TypeScript -> Typed AST -> Semantic Analysis -> Typed IR -> Verified Bytecode -> TSVM"
    ));
    assert!(output.contains("Generated JavaScript: no"));
    assert!(output.contains("Initial demo console: 150"));
    assert!(output.contains("Script loader console: 42"));
    assert!(output.contains("DOM #app after fetch: hello from TSVM"));
    assert!(output.contains("Cross-origin fetch: blocked"));
    assert!(output.contains("Benchmark snapshot"));
    assert!(output.contains("initial-demo"));
    assert!(output.contains("function-calls"));
    assert!(output.contains("object-mutation"));
}
