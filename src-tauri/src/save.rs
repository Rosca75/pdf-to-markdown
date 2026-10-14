//! Output naming and atomic saving of converted Markdown.
//!
//! Policy: never overwrite. `report.pdf` becomes `report.md`; when that name is
//! taken (on disk or earlier in the same batch) we use `report (1).md`,
//! `report (2).md`, and so on.

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One file the user asked to save.
#[derive(Debug, Clone, Deserialize)]
pub struct SaveRequest {
    pub id: String,
    /// Source file name as shown in the UI (for example `report.pdf`).
    pub name: String,
}

/// A request resolved against the extraction results.
#[derive(Debug, Clone)]
pub struct SaveJob<'a> {
    pub id: String,
    pub name: String,
    /// `None` when the file has no extracted Markdown (not ready or removed).
    pub markdown: Option<&'a str>,
}

/// Outcome for one file, serialised to the frontend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SaveOutcome {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// File stem to use for the output of the source called `name`.
///
/// Strips the extension, replaces characters that are illegal in file names on
/// any supported OS, and avoids Windows reserved device names.
pub fn output_stem(name: &str) -> String {
    // The UI sends a bare file name, but be defensive about separators.
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = match base.rfind('.') {
        Some(i) => &base[..i],
        None => base,
    };
    let cleaned: String = stem
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if cleaned.is_empty() {
        return "document".to_string();
    }
    // Windows treats `CON`, `CON.txt` and `con.report` alike: the device
    // name is matched on the part before the first dot.
    let device = cleaned.split('.').next().unwrap_or(cleaned).trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(device)) {
        return format!("_{cleaned}");
    }
    cleaned.to_string()
}

/// File name for collision attempt `n` (0 = no suffix).
pub fn candidate_name(stem: &str, n: usize) -> String {
    if n == 0 {
        format!("{stem}.md")
    } else {
        format!("{stem} ({n}).md")
    }
}

/// Tracks names already handed out in one batch (case-insensitive, because
/// Windows and macOS file systems are).
#[derive(Debug, Default)]
pub struct BatchNames(HashSet<String>);

impl BatchNames {
    fn contains(&self, name: &str) -> bool {
        self.0.contains(&name.to_lowercase())
    }
    fn insert(&mut self, name: &str) {
        self.0.insert(name.to_lowercase());
    }
}

/// First free output path for `stem` in `dir`, skipping names that exist on
/// disk or were already used in this batch. Starts searching at suffix `from`.
fn free_name(dir: &Path, stem: &str, used: &BatchNames, from: usize) -> (usize, String) {
    let mut n = from;
    loop {
        let name = candidate_name(stem, n);
        if !used.contains(&name) && !dir.join(&name).exists() {
            return (n, name);
        }
        n += 1;
    }
}

/// Human wording for a failed write.
pub fn describe_io_error(dir: &Path, err: &io::Error) -> String {
    let d = dir.display();
    match err.kind() {
        io::ErrorKind::PermissionDenied => {
            format!("Cannot write to {d}: access denied. Choose another folder.")
        }
        io::ErrorKind::NotFound => {
            format!("Cannot write to {d}: the folder does not exist. Choose another folder.")
        }
        io::ErrorKind::StorageFull => {
            format!(
                "Cannot write to {d}: the disk is full. Free some space or choose another folder."
            )
        }
        io::ErrorKind::ReadOnlyFilesystem => {
            format!("Cannot write to {d}: the location is read-only. Choose another folder.")
        }
        _ => format!("Cannot write to {d}: {err}. Choose another folder."),
    }
}

/// Write `content` to a fresh `stem[ (n)].md` in `dir` atomically: the data is
/// written to a temp file in the same directory and renamed into place without
/// clobbering. If another process grabs the name meanwhile, the next free name
/// is tried.
fn write_unique(
    dir: &Path,
    stem: &str,
    content: &str,
    used: &mut BatchNames,
) -> Result<PathBuf, io::Error> {
    let mut tmp = tempfile::Builder::new()
        .prefix(".pdf2md-")
        .suffix(".tmp")
        .tempfile_in(dir)?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;
    // Temp files are created 0600; a saved document should be readable
    // like any other file the user creates.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o644))?;
    }
    tmp.as_file().sync_all()?;

    let mut from = 0;
    loop {
        let (n, name) = free_name(dir, stem, used, from);
        let target = dir.join(&name);
        match tmp.persist_noclobber(&target) {
            Ok(_) => {
                used.insert(&name);
                return Ok(target);
            }
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
                tmp = e.file;
                from = n + 1;
            }
            Err(e) => return Err(e.error),
        }
    }
}

/// Save every job into `dir`. One file failing never stops the others.
pub fn save_batch(dir: &Path, jobs: &[SaveJob<'_>]) -> Vec<SaveOutcome> {
    let dir_problem = match std::fs::metadata(dir) {
        Ok(m) if m.is_dir() => None,
        Ok(_) => Some(format!(
            "Cannot write to {}: it is not a folder. Choose another folder.",
            dir.display()
        )),
        Err(e) => Some(describe_io_error(dir, &e)),
    };

    let mut used = BatchNames::default();
    jobs.iter()
        .map(|job| {
            let fail = |error: String| SaveOutcome {
                id: job.id.clone(),
                path: None,
                error: Some(error),
            };
            if let Some(problem) = &dir_problem {
                return fail(problem.clone());
            }
            let Some(markdown) = job.markdown else {
                return fail(format!(
                    "{} has no extracted text to save. Wait for it to finish, or add it again.",
                    job.name
                ));
            };
            match write_unique(dir, &output_stem(&job.name), markdown, &mut used) {
                Ok(path) => SaveOutcome {
                    id: job.id.clone(),
                    path: Some(path.to_string_lossy().into_owned()),
                    error: None,
                },
                Err(e) => fail(describe_io_error(dir, &e)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn job<'a>(id: &str, name: &str, md: &'a str) -> SaveJob<'a> {
        SaveJob {
            id: id.into(),
            name: name.into(),
            markdown: Some(md),
        }
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn stem_strips_extension() {
        assert_eq!(output_stem("report.pdf"), "report");
        assert_eq!(output_stem("Report.PDF"), "Report");
        assert_eq!(output_stem("a.b.pdf"), "a.b");
        assert_eq!(output_stem("noext"), "noext");
    }

    #[test]
    fn stem_is_sanitised() {
        assert_eq!(output_stem("a:b?c.pdf"), "a_b_c");
        assert_eq!(output_stem("dir/sub\\x.pdf"), "x");
        assert_eq!(output_stem(".pdf"), "document");
        assert_eq!(output_stem("  .pdf"), "document");
        assert_eq!(output_stem("con.pdf"), "_con");
        assert_eq!(output_stem("Müller Büro.pdf"), "Müller Büro");
    }

    #[test]
    fn candidate_names() {
        assert_eq!(candidate_name("x", 0), "x.md");
        assert_eq!(candidate_name("x", 2), "x (2).md");
    }

    #[test]
    fn reserved_device_names_are_prefixed_even_with_dots() {
        assert_eq!(output_stem("CON.pdf"), "_CON");
        assert_eq!(output_stem("con.report.pdf"), "_con.report");
        assert_eq!(output_stem("lpt1 .final.pdf"), "_lpt1 .final");
        assert_eq!(output_stem("console.pdf"), "console");
    }

    #[test]
    fn saves_with_md_extension() {
        let dir = tempfile::tempdir().unwrap();
        let out = save_batch(dir.path(), &[job("1", "report.pdf", "# Hi")]);
        assert_eq!(out.len(), 1);
        let p = out[0].path.as_ref().unwrap();
        assert!(p.ends_with("report.md"));
        assert_eq!(fs::read_to_string(p).unwrap(), "# Hi");
        // No temp files left behind.
        assert_eq!(file_names(dir.path()), vec!["report.md"]);
    }

    #[test]
    fn existing_file_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("report.md"), "old").unwrap();
        fs::write(dir.path().join("report (1).md"), "old1").unwrap();
        let out = save_batch(dir.path(), &[job("1", "report.pdf", "new")]);
        let p = out[0].path.as_ref().unwrap();
        assert!(p.ends_with("report (2).md"), "got {p}");
        assert_eq!(
            fs::read_to_string(dir.path().join("report.md")).unwrap(),
            "old"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("report (1).md")).unwrap(),
            "old1"
        );
        assert_eq!(fs::read_to_string(p).unwrap(), "new");
    }

    #[test]
    fn collisions_within_batch() {
        let dir = tempfile::tempdir().unwrap();
        let out = save_batch(
            dir.path(),
            &[
                job("a", "report.pdf", "A"),
                job("b", "report.pdf", "B"),
                job("c", "Report.PDF", "C"),
            ],
        );
        let names: Vec<String> = out
            .iter()
            .map(|o| {
                Path::new(o.path.as_ref().unwrap())
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, vec!["report.md", "report (1).md", "Report (2).md"]);
        assert_eq!(
            fs::read_to_string(dir.path().join("report.md")).unwrap(),
            "A"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("report (1).md")).unwrap(),
            "B"
        );
    }

    #[test]
    fn missing_markdown_is_an_error_but_others_save() {
        let dir = tempfile::tempdir().unwrap();
        let jobs = [
            SaveJob {
                id: "a".into(),
                name: "gone.pdf".into(),
                markdown: None,
            },
            job("b", "ok.pdf", "ok"),
        ];
        let out = save_batch(dir.path(), &jobs);
        assert!(out[0].path.is_none());
        assert!(out[0].error.as_ref().unwrap().contains("gone.pdf"));
        assert!(out[1].path.is_some());
        assert_eq!(file_names(dir.path()), vec!["ok.md"]);
    }

    #[test]
    fn missing_folder_gives_actionable_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let out = save_batch(&missing, &[job("1", "a.pdf", "x"), job("2", "b.pdf", "y")]);
        for o in &out {
            let e = o.error.as_ref().unwrap();
            assert!(e.contains("Cannot write to"), "{e}");
            assert!(e.contains("does not exist"), "{e}");
            assert!(e.contains("Choose another folder"), "{e}");
        }
    }

    #[test]
    fn target_that_is_a_file_gives_error() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("file.txt");
        fs::write(&f, "x").unwrap();
        let out = save_batch(&f, &[job("1", "a.pdf", "x")]);
        assert!(out[0].error.as_ref().unwrap().contains("not a folder"));
    }

    #[test]
    fn io_error_wording() {
        let d = Path::new("C:\\locked");
        let e = io::Error::from(io::ErrorKind::PermissionDenied);
        let s = describe_io_error(d, &e);
        assert!(s.contains("access denied"));
        assert!(s.contains("C:\\locked"));
    }

    #[test]
    fn unicode_content_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let text = "# Zażółć gęślą jaźń — 日本語\n";
        let out = save_batch(dir.path(), &[job("1", "u.pdf", text)]);
        assert_eq!(
            fs::read_to_string(out[0].path.as_ref().unwrap()).unwrap(),
            text
        );
    }
}
