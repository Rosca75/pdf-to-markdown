//! Runs the engine over every corpus PDF and checks the manifest expectations.
//!
//! All failures are collected and reported together, and every produced
//! Markdown file is written to `target/corpus-out/` for inspection.

use std::path::{Path, PathBuf};

use pdf2md_engine::{convert_path, CancelToken, Options};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    case: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    file: String,
    #[serde(default)]
    about: String,
    password: Option<String>,
    expect_error: Option<String>,
    #[serde(default)]
    must_contain: Vec<String>,
    #[serde(default)]
    must_not_contain: Vec<String>,
    #[serde(default)]
    in_order: Vec<String>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn corpus_expectations() {
    let root = repo_root();
    let corpus = root.join("tests/corpus");
    let text = std::fs::read_to_string(corpus.join("manifest.toml")).expect("manifest");
    let manifest: Manifest = match toml::from_str(&text) {
        Ok(m) => m,
        Err(e) => panic!("manifest.toml: {} at {:?}", e.message(), e.span()),
    };
    let out_dir = root.join("target/corpus-out");
    let _ = std::fs::create_dir_all(&out_dir);
    let only = std::env::var("CORPUS_CASE").ok();

    let mut failures = Vec::new();
    let mut checked = 0;
    for case in &manifest.case {
        if only.as_deref().is_some_and(|o| !case.file.contains(o)) {
            continue;
        }
        checked += 1;
        let opts = Options { password: case.password.clone(), threads: None };
        let label = format!("{} (pw={:?})", case.file, case.password);
        let result = convert_path(&corpus.join(&case.file), &opts, &|_| {}, &CancelToken::new());
        match (&case.expect_error, result) {
            (Some(code), Ok(_)) => failures.push(format!("{label}: expected error `{code}`, got success")),
            (Some(code), Err(e)) if e.code() != code => {
                failures.push(format!("{label}: expected error `{code}`, got `{}` ({e})", e.code()))
            }
            (Some(_), Err(_)) => {}
            (None, Err(e)) => failures.push(format!("{label}: unexpected error `{}`: {e}", e.code())),
            (None, Ok(conv)) => {
                let suffix = if case.password.is_some() { ".pw" } else { "" };
                let _ = std::fs::write(out_dir.join(format!("{}{suffix}.md", case.file)), &conv.markdown);
                let md = &conv.markdown;
                for s in &case.must_contain {
                    if !md.contains(s.as_str()) {
                        failures.push(format!("{label}: missing {s:?}"));
                    }
                }
                for s in &case.must_not_contain {
                    if md.contains(s.as_str()) {
                        failures.push(format!("{label}: must not contain {s:?}"));
                    }
                }
                let mut pos = 0;
                for s in &case.in_order {
                    match md[pos..].find(s.as_str()) {
                        Some(i) => pos += i + s.len(),
                        None => {
                            failures.push(format!("{label}: {s:?} not found in order"));
                            break;
                        }
                    }
                }
            }
        }
        let _ = &case.about;
    }
    assert!(checked > 0, "no corpus case selected");
    assert!(failures.is_empty(), "{} corpus failure(s):\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_corpus_pdf_converts_without_panic() {
    let corpus = repo_root().join("tests/corpus");
    for entry in std::fs::read_dir(&corpus).expect("corpus dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "pdf") {
            let r = std::panic::catch_unwind(|| convert_path(&path, &Options::default(), &|_| {}, &CancelToken::new()));
            assert!(r.is_ok(), "panic converting {}", path.display());
        }
    }
}
