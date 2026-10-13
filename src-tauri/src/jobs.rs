//! Background extraction: bounded concurrency, cancellation, panic isolation.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use pdf2md_engine::{CancelToken, Conversion, ConvertError, Options, Progress};
use serde::Serialize;

/// Files converting at the same time. The engine already uses all cores for
/// the pages of one file, so more than two only costs memory.
pub const MAX_CONCURRENT: usize = 2;

/// Minimum spacing between progress events for one file (about 20 per second).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProgressEvent {
    pub id: String,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoneEvent {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pages: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
}

/// Something the worker wants the frontend to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Progress(ProgressEvent),
    Done(DoneEvent),
}

struct Entry {
    generation: u64,
    cancel: CancelToken,
    markdown: Option<Arc<String>>,
}

/// Counting semaphore whose waiters give up when their file is cancelled.
struct Slots {
    free: Mutex<usize>,
    cv: Condvar,
}

struct Permit<'a>(&'a Slots);

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        *lock(&self.0.free) += 1;
        self.0.cv.notify_one();
    }
}

impl Slots {
    fn new(n: usize) -> Self {
        Self {
            free: Mutex::new(n),
            cv: Condvar::new(),
        }
    }

    /// Wait for a free slot; `None` if `cancel` fires first.
    fn acquire(&self, cancel: &CancelToken) -> Option<Permit<'_>> {
        let mut free = lock(&self.free);
        loop {
            if cancel.is_cancelled() {
                return None;
            }
            if *free > 0 {
                *free -= 1;
                return Some(Permit(self));
            }
            free = self
                .cv
                .wait_timeout(free, Duration::from_millis(100))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}

/// Locks even if a previous holder panicked: our state stays consistent.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Shared application state.
pub struct AppState {
    files: Mutex<HashMap<String, Entry>>,
    slots: Slots,
    next_generation: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(MAX_CONCURRENT)
    }
}

/// What `begin` hands to the worker.
pub struct Run {
    pub generation: u64,
    pub cancel: CancelToken,
}

impl AppState {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
            slots: Slots::new(max_concurrent),
            next_generation: AtomicU64::new(1),
        }
    }

    /// Register a new run for `id`, cancelling and replacing any previous one.
    pub fn begin(&self, id: &str) -> Run {
        let run = Run {
            generation: self.next_generation.fetch_add(1, Ordering::Relaxed),
            cancel: CancelToken::new(),
        };
        let entry = Entry {
            generation: run.generation,
            cancel: run.cancel.clone(),
            markdown: None,
        };
        if let Some(old) = lock(&self.files).insert(id.to_string(), entry) {
            old.cancel.cancel();
        }
        run
    }

    /// Cancel a running conversion and drop its Markdown.
    pub fn remove(&self, id: &str) {
        if let Some(old) = lock(&self.files).remove(id) {
            old.cancel.cancel();
        }
    }

    pub fn markdown(&self, id: &str) -> Option<Arc<String>> {
        lock(&self.files).get(id).and_then(|e| e.markdown.clone())
    }

    /// Store the result if `generation` is still the current run for `id`.
    fn finish(&self, id: &str, generation: u64, markdown: Option<String>) -> bool {
        let mut files = lock(&self.files);
        match files.get_mut(id) {
            Some(e) if e.generation == generation && !e.cancel.is_cancelled() => {
                e.markdown = markdown.map(Arc::new);
                true
            }
            _ => false,
        }
    }

    /// Run the whole job on the calling (blocking) thread. Blocks while the
    /// maximum number of other files are converting. Emits nothing if the run
    /// was cancelled or superseded.
    pub fn run_extraction(
        &self,
        id: &str,
        run: &Run,
        path: &Path,
        password: Option<String>,
        emit: &(dyn Fn(Event) + Sync),
    ) {
        let Some(_permit) = self.slots.acquire(&run.cancel) else {
            return;
        };
        // Tell the UI this file left the queue (total unknown until page 1).
        emit(Event::Progress(ProgressEvent {
            id: id.to_string(),
            done: 0,
            total: 0,
        }));

        let result = convert_isolated(path, password, id, run, emit);

        let event = match result {
            Ok(conv) => {
                let pages = conv.pages;
                let warnings = format_warnings(&conv);
                if !self.finish(id, run.generation, Some(conv.markdown)) {
                    return;
                }
                DoneEvent {
                    id: id.to_string(),
                    ok: true,
                    pages: Some(pages),
                    warnings: Some(warnings),
                    error: None,
                }
            }
            Err(err) => {
                if run.cancel.is_cancelled() || !self.finish(id, run.generation, None) {
                    return;
                }
                DoneEvent {
                    id: id.to_string(),
                    ok: false,
                    pages: None,
                    warnings: None,
                    error: Some(ErrorInfo {
                        code: err.code.to_string(),
                        message: err.message,
                        hint: err.hint,
                    }),
                }
            }
        };
        emit(Event::Done(event));
    }
}

struct Failure {
    code: &'static str,
    message: String,
    hint: String,
}

impl From<ConvertError> for Failure {
    fn from(e: ConvertError) -> Self {
        Self {
            code: e.code(),
            message: e.message(),
            hint: e.hint().to_string(),
        }
    }
}

fn format_warnings(conv: &Conversion) -> Vec<String> {
    conv.warnings
        .iter()
        .map(|w| match w.page {
            Some(p) => format!("Page {}: {}", p + 1, w.message),
            None => w.message.clone(),
        })
        .collect()
}

/// Call the engine so that neither a panic nor a progress flood can hurt the app.
fn convert_isolated(
    path: &Path,
    password: Option<String>,
    id: &str,
    run: &Run,
    emit: &(dyn Fn(Event) + Sync),
) -> Result<Conversion, Failure> {
    let opts = Options {
        password,
        threads: None,
    };
    let last_sent = Mutex::new(None::<Instant>);
    let progress = |p: Progress| {
        if run.cancel.is_cancelled() {
            return;
        }
        let mut last = lock(&last_sent);
        let due = p.done >= p.total || last.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL);
        if due {
            *last = Some(Instant::now());
            drop(last);
            emit(Event::Progress(ProgressEvent {
                id: id.to_string(),
                done: p.done,
                total: p.total,
            }));
        }
    };

    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        pdf2md_engine::convert_path(path, &opts, &progress, &run.cancel)
    }));
    match outcome {
        Ok(Ok(conv)) => Ok(conv),
        Ok(Err(e)) => Err(e.into()),
        Err(payload) => Err(Failure {
            code: "internal",
            message: format!(
                "An unexpected error occurred while reading this PDF ({}).",
                panic_text(payload.as_ref())
            ),
            hint: "Try again. If it keeps failing, re-save the PDF with a PDF viewer and add the new copy.".to_string(),
        }),
    }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown cause".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::thread;

    fn collect(state: &AppState, id: &str, run: &Run, path: &Path) -> Vec<Event> {
        let events = Mutex::new(Vec::new());
        state.run_extraction(id, run, path, None, &|e| lock(&events).push(e));
        events.into_inner().unwrap()
    }

    #[test]
    fn missing_file_reports_not_found() {
        let state = AppState::default();
        let run = state.begin("a");
        let events = collect(&state, "a", &run, Path::new("definitely/missing.pdf"));
        match events.last().unwrap() {
            Event::Done(d) => {
                assert!(!d.ok);
                let e = d.error.as_ref().unwrap();
                assert_eq!(e.code, "not_found");
                assert!(!e.hint.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(state.markdown("a").is_none());
    }

    #[test]
    fn non_pdf_reports_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.pdf");
        std::fs::write(&f, b"this is not a pdf at all").unwrap();
        let state = AppState::default();
        let run = state.begin("a");
        let events = collect(&state, "a", &run, &f);
        assert!(matches!(events.last(), Some(Event::Done(d)) if !d.ok));
    }

    #[test]
    fn cancelled_run_is_silent() {
        let state = AppState::default();
        let run = state.begin("a");
        state.remove("a");
        let events = collect(&state, "a", &run, Path::new("missing.pdf"));
        assert!(events.is_empty());
    }

    #[test]
    fn replaced_run_cancels_the_old_one() {
        let state = AppState::default();
        let first = state.begin("a");
        let second = state.begin("a");
        assert!(first.cancel.is_cancelled());
        assert!(!second.cancel.is_cancelled());
        assert!(!state.finish("a", first.generation, Some("old".into())));
        assert!(state.finish("a", second.generation, Some("new".into())));
        assert_eq!(state.markdown("a").unwrap().as_str(), "new");
    }

    #[test]
    fn remove_drops_markdown() {
        let state = AppState::default();
        let run = state.begin("a");
        assert!(state.finish("a", run.generation, Some("x".into())));
        state.remove("a");
        assert!(state.markdown("a").is_none());
    }

    #[test]
    fn slots_bound_concurrency_and_release() {
        let slots = Arc::new(Slots::new(2));
        let token = CancelToken::new();
        let p1 = slots.acquire(&token).unwrap();
        let _p2 = slots.acquire(&token).unwrap();

        // A third waiter blocks until a permit is dropped.
        let got = Arc::new(AtomicUsize::new(0));
        let (s, g) = (slots.clone(), got.clone());
        let h = thread::spawn(move || {
            let t = CancelToken::new();
            let _p = s.acquire(&t).unwrap();
            g.fetch_add(1, Ordering::SeqCst);
        });
        thread::sleep(Duration::from_millis(250));
        assert_eq!(got.load(Ordering::SeqCst), 0);
        drop(p1);
        h.join().unwrap();
        assert_eq!(got.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn queued_waiter_can_be_cancelled() {
        let slots = Slots::new(0);
        let token = CancelToken::new();
        token.cancel();
        assert!(slots.acquire(&token).is_none());
    }

    #[test]
    fn panic_text_extracts_message() {
        let p = panic::catch_unwind(|| panic!("boom {}", 1)).unwrap_err();
        assert_eq!(panic_text(p.as_ref()), "boom 1");
        let p = panic::catch_unwind(|| panic!("static text")).unwrap_err();
        assert_eq!(panic_text(p.as_ref()), "static text");
    }

    #[test]
    fn events_serialise_as_the_frontend_expects() {
        let d = DoneEvent {
            id: "f1".into(),
            ok: true,
            pages: Some(3),
            warnings: Some(vec!["Page 2: skipped".into()]),
            error: None,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["pages"], 3);
        assert!(v.get("error").is_none());
    }
}
