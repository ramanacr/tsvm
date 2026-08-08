#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use tsvm_lexer::lex;

fn main() -> ExitCode {
    let corpus_root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tests/fixtures/lexer"));

    match run_corpus(&corpus_root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run_corpus(root: &Path) -> Result<(), String> {
    let valid = root.join("valid");
    let invalid = root.join("invalid");

    for path in list_files(&valid)? {
        let source =
            fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        lex(&source).map_err(|err| format!("{} should lex but failed: {err}", path.display()))?;
        mutate_and_lex(&source);
    }

    for path in list_files(&invalid)? {
        let source =
            fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        if lex(&source).is_ok() {
            return Err(format!(
                "{} should fail but lexed successfully",
                path.display()
            ));
        }
        mutate_and_lex(&source);
    }

    Ok(())
}

fn list_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    if !root.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))? {
        let path = entry
            .map_err(|err| format!("{}: {err}", root.display()))?
            .path();
        if path.is_file() {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn mutate_and_lex(source: &str) {
    let mut bytes = source.as_bytes().to_vec();
    let mut state = 0x5EED_u64 ^ bytes.len() as u64;

    for _ in 0..64 {
        if bytes.is_empty() {
            break;
        }

        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = (state as usize) % bytes.len();
        bytes[index] ^= ((state >> 24) as u8).max(1);

        if let Ok(mutated) = std::str::from_utf8(&bytes) {
            let _ = lex(mutated);
        }
    }
}
