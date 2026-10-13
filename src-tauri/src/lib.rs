//! PDF2MD desktop shell: Tauri commands and events around `pdf2md-engine`.

mod jobs;
mod save;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{async_runtime, AppHandle, Emitter, Manager, State};

use jobs::{AppState, Event};
use save::{SaveJob, SaveOutcome, SaveRequest};

const EVENT_PROGRESS: &str = "extract://progress";
const EVENT_DONE: &str = "extract://done";

/// Start (or restart) extraction of `path` under the frontend's `id`.
/// Returns immediately; results arrive as `extract://progress` and
/// `extract://done` events.
#[tauri::command]
async fn extract(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    path: String,
    password: Option<String>,
) -> Result<(), String> {
    let run = state.begin(&id);
    async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let emit = |event: Event| {
            // The only failure is a closed window; nothing useful to do then.
            let _ = match event {
                Event::Progress(p) => app.emit(EVENT_PROGRESS, p),
                Event::Done(d) => app.emit(EVENT_DONE, d),
            };
        };
        state.run_extraction(&id, &run, &PathBuf::from(path), password, &emit);
    });
    Ok(())
}

/// Markdown of a finished file.
#[tauri::command]
async fn get_markdown(state: State<'_, AppState>, id: String) -> Result<String, String> {
    state
        .markdown(&id)
        .map(|m| m.as_str().to_owned())
        .ok_or_else(|| "This file has no extracted text yet.".to_string())
}

/// Forget a file: cancel it if it is running and free its Markdown.
#[tauri::command]
async fn remove(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.remove(&id);
    Ok(())
}

/// Write the Markdown of each requested file into `dir`.
#[tauri::command]
async fn save_all(
    state: State<'_, AppState>,
    dir: String,
    items: Vec<SaveRequest>,
) -> Result<Vec<SaveOutcome>, String> {
    let resolved: Vec<(SaveRequest, Option<Arc<String>>)> = items
        .into_iter()
        .map(|r| {
            let md = state.markdown(&r.id);
            (r, md)
        })
        .collect();
    async_runtime::spawn_blocking(move || {
        let jobs: Vec<SaveJob<'_>> = resolved
            .iter()
            .map(|(r, md)| SaveJob {
                id: r.id.clone(),
                name: r.name.clone(),
                markdown: md.as_ref().map(|m| m.as_str()),
            })
            .collect();
        save::save_batch(Path::new(&dir), &jobs)
    })
    .await
    .map_err(|e| format!("Saving was interrupted unexpectedly ({e}). Try again."))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            extract,
            get_markdown,
            remove,
            save_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running PDF2MD");
}
