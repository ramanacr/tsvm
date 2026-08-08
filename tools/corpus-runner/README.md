# Corpus Runner

The current M1 corpus runner lives in the lexer crate:

```sh
cargo run -p tsvm-lexer --bin lexer_corpus_runner -- tests/fixtures/lexer
```

It loads valid and invalid lexer fixtures, verifies the expected outcome, and
runs a deterministic mutation smoke pass to catch panics in scanner code. It is
not a replacement for coverage-guided fuzzing, but it gives CI a fast fuzz-like
guard until `cargo-fuzz` targets are introduced.

