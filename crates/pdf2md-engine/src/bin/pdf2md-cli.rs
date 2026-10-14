//! Command-line front end: `pdf2md-cli <input.pdf> [output.md]`.

use std::path::PathBuf;
use std::process::ExitCode;

use pdf2md_engine::{convert_path, CancelToken, Options};

// Same allocator as the desktop app (see docs/performance.md).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("usage: pdf2md-cli <input.pdf> [output.md]");
        return ExitCode::from(2);
    };
    let output = args.next().map(PathBuf::from);
    match convert_path(&input, &Options::default(), &|_| {}, &CancelToken::new()) {
        Ok(conv) => {
            for w in &conv.warnings {
                eprintln!("warning: {}", w.message);
            }
            match output {
                Some(out) => {
                    if let Err(e) = std::fs::write(&out, conv.markdown) {
                        eprintln!("error: cannot write {}: {e}", out.display());
                        return ExitCode::FAILURE;
                    }
                }
                None => print!("{}", conv.markdown),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error [{}]: {} {}", e.code(), e.message(), e.hint());
            ExitCode::FAILURE
        }
    }
}
