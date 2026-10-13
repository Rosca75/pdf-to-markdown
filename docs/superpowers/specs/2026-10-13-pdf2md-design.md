# PDF2MD — design

Date: 2026-10-13. Status: approved through the clarification phase (decisions below are the owner's answers; everything else was decided autonomously).

## Goal

A standalone desktop app that converts one or more PDFs to Markdown. Rust conversion engine, Tauri v2 shell, web UI. It ships as three portable artifacts built by GitHub Actions: a portable `.exe` (Windows 11), an AppImage (Ubuntu 24.04), and a zipped universal `.app` (latest macOS).

## Owner decisions (clarification phase)

| Topic | Decision |
|---|---|
| Output location | A folder picker opens once per batch on Convert. It defaults to the last folder used (remembered per user). |
| Name collision | `report.md` exists → write `report (1).md`, `report (2).md`, … Never overwrite. |
| Batch errors | Continue with the remaining files and mark each failed file with an actionable message. |
| Preview | File list in the left panel; clicking a file previews it. The preview shows the raw Markdown source in monospace, read-only, with a Copy button. |
| Pipeline | Extract on load, in the background, with the progress bar tracking it. Convert only writes to disk. |
| Signing | Unsigned. The README documents SmartScreen / Gatekeeper bypass. |
| macOS | Universal binary (arm64 + x86_64). |
| Markdown features | Headings, paragraphs, lists, bold/italic, tables, hyperlinks. |
| Running headers/footers/page numbers | Stripped. |
| Encrypted PDFs | Empty user password → opened silently. Otherwise the file shows "Password required" with an inline password field. |
| Release | A `v*` tag push builds all three OSes and publishes a public GitHub Release with the artifacts. |
| Page breaks | Seamless: text flows across pages, and words hyphenated across a page break are rejoined. No page markers. |
| Name | PDF2MD. Binary `pdf2md`, bundle `PDF2MD.app`, identifier `io.github.rosca75.pdf2md`. |

Out of scope: OCR, images/shapes/diagrams in the output (dropped), editing, legacy OS versions.

## Repository layout (standard Tauri v2 + Cargo workspace)

```
Cargo.toml                 workspace: crates/*, src-tauri
crates/pdf2md-engine/      conversion engine (lib + `pdf2md` CLI bin used for corpus runs and benches)
  src/                     see "Engine architecture"
  tests/                   integration tests over tests/corpus
  benches/                 criterion benches
crates/pdf-fixtures/       dev-only raw-PDF writer + corpus generator (`gen-corpus` bin)
tests/corpus/              generated + committed corpus PDFs and expectation manifest
src-tauri/                 Tauri v2 app shell (commands, events, state)
src/                       frontend (TypeScript + Vite, no framework)
index.html, package.json, vite.config.ts, vitest config
.github/workflows/         ci.yml (tests on push/PR), release.yml (tag v*)
docs/                      spec, plan, perf report, corpus report
```

## pdf_oxide verification

To be completed by the spike before the engine is built. It must confirm that per-page span extraction yields text, bbox, font size, bold/italic and monospace flags (verified by reading the 0.3.78 source: `TextSpan` carries all of these), that link annotations with URI actions are available (`get_annotations` → `LinkAction::Uri`), and that encrypted files can be authenticated (`authenticate(password)`). It also measures how the built-in `to_markdown` behaves on the corpus. Known limitation: `PdfDocument` is neither `Send` nor `Sync`, so parallelism has to open a separate document per worker thread. Spike results and the resulting decision are recorded in `docs/pdf_oxide-evaluation.md`.

## Engine architecture (`pdf2md-engine`)

The engine owns its pipeline as pure stages over its own types. pdf_oxide types never leave `extract`.

1. `source` — open from path or bytes; detect not-a-PDF (magic header within the first 1 KiB), password handling, typed `ConvertError`.
2. `extract` — pdf_oxide adapter: page → `PageContent { index, width, height, spans: Vec<Span>, links: Vec<Link>, rules: Vec<Rule> }`. `Span { text, bbox, font_size, bold, italic, mono, artifact }`. Pages run in parallel (per-worker document handles), and results are reassembled in page order.
3. `normalize` — Unicode clean-up per span: ligatures, soft hyphens, NBSP, control chars, private-use/U+FFFD reporting, NFC.
4. `layout` — spans → lines (baseline clustering, x-sorted, gap-based space insertion) → columns → blocks (paragraph segmentation by leading gap, indent and font change), with reading order per page.
5. `furniture` — cross-page detection of running headers, footers and page numbers (normalized text of top/bottom band lines that recur on ≥ 50% of pages with digits masked; bare page numbers). Removed before classification.
6. `classify` — document font statistics → body size; heading levels (size rank + bold + short-line heuristics), list items (bullet glyphs, `1.`/`a)`/`(i)` markers, indent nesting), code blocks (monospace runs), tables (`tables` sub-module: ruled-grid detection from rules, falling back to column-aligned text grids).
7. `flow` — cross-page joining: a paragraph that ends a page without terminal punctuation continues the first paragraph of the next page; trailing-hyphen dehyphenation (a hyphen at the end of a line or page joins a lowercase continuation).
8. `render` — block model → CommonMark/GFM text. Inline emphasis merging, escaping of Markdown-significant characters, links `[text](url)` from link-annotation rects intersecting spans, GFM tables with pipe escaping.

Public API:

```rust
pub struct Options { pub password: Option<String>, pub threads: Option<usize> }
pub struct Conversion { pub markdown: String, pub pages: usize, pub warnings: Vec<Warning> }
pub fn convert_path(path: &Path, opts: &Options, progress: &dyn Fn(Progress) + Sync, cancel: &CancelToken) -> Result<Conversion, ConvertError>;
pub fn convert_bytes(bytes: Vec<u8>, ...) -> Result<Conversion, ConvertError>;
```

`ConvertError` variants: `NotFound`, `PermissionDenied`, `NotAPdf`, `Corrupt(detail)`, `PasswordRequired`, `WrongPassword`, `NoTextLayer` (all pages empty: scanned document, OCR not supported), `Cancelled`, `Io(detail)`. Each carries a stable `code()` and a user-facing `message()` + `hint()`. Every engine call is panic-isolated (`catch_unwind` at the page level), so a panic inside pdf_oxide becomes `Corrupt` for that page/file and never crashes the app. A single failing page degrades to a warning and its text is skipped. The file fails only if every page fails.

## Tauri shell (`src-tauri`)

State: `Mutex<HashMap<FileId, Entry>>` holding the Markdown of finished files. The Markdown is kept once in Rust and fetched by the UI on demand.

Commands:
- `extract(id, path, password?)` — spawns work on a bounded worker pool (one file at a time per worker; the engine parallelizes pages inside a file). Emits `extract://progress {id, page, total}` and `extract://done {id, ok, pages, warnings, error?: {code, message, hint}}`.
- `get_markdown(id) -> String`
- `remove(id)` (frees memory and cancels if running)
- `save_all(dir, items: [{id, name}]) -> [{id, path?|error?}]` — applies collision suffixing and atomic write (temp file + rename).

The frontend uses the dialog plugin (open PDFs, pick a folder) and the webview drag-drop event (`onDragDropEvent` gives absolute paths). Non-PDF drops are rejected client-side with a message.

## Frontend

Vanilla TypeScript + Vite; Vitest + jsdom for tests. A thin `api.ts` wraps every Tauri call so tests can mock it. A pure `state.ts` store (files, selection, progress) with reducer-style transitions makes the UI logic unit-testable without the DOM. The views render from state.

Layout: header (app name), two panels, and a bottom bar.
- Left: drop zone (click or drag), file list (name, pages, status chip: queued / extracting n% / ready / needs password / error), per-file remove, overall progress bar, and inline error text with a hint (e.g. "Encrypted — enter the password", "Not a PDF — choose a .pdf file", "Scanned document — no text layer; OCR is not supported").
- Right: Markdown preview of the selected file (defaults to the first loaded), monospace, read-only, Copy button; it shows an empty state when nothing is loaded.
- Bottom: "Convert to Markdown". Disabled until at least one file is ready. Results summary: "3 saved to …, 1 failed".
- Follows the OS light/dark theme. Keyboard accessible.

## Performance

Baseline first (CLI `pdf2md bench` over generated large documents: 500-page text-heavy, 100-page mixed with tables/images), measured on the dev laptop (i7-6600U, 2C/4T, 8 GB — representative of a standard business laptop). Targets are set from the baseline and recorded in `docs/performance.md` with candidate techniques, chosen techniques and before/after numbers. Criterion benches guard regressions. Peak memory is measured with a counting global allocator in the bench binary.

## Testing

- Engine unit tests per stage (pure functions over synthetic spans).
- Integration tests: every corpus PDF converts without panic; per-file expectations in `tests/corpus/manifest.toml` (must-contain / must-not-contain strings, heading lines, expected error code).
- Fixture crate tests (generated PDFs parse).
- Frontend: Vitest unit tests for state, the API wrapper and views.
- Tauri shell: unit tests for collision naming and save logic.
- CI: `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`, `npm test`, on ubuntu/windows/macos.

## Distribution

`release.yml` on `v*` tags, with a matrix of windows-latest, ubuntu-24.04 and macos-latest:
- Windows: `tauri build --no-bundle` → `target/release/pdf2md.exe` renamed `PDF2MD_<ver>_windows_x64_portable.exe` (WebView2 is part of Windows 11).
- Ubuntu: `tauri build --bundles appimage` → `PDF2MD_<ver>_linux_x86_64.AppImage`.
- macOS: `tauri build --target universal-apple-darwin --bundles app` → `ditto -c -k --keepParent` → `PDF2MD_<ver>_macos_universal.zip`.
- A final job creates the GitHub Release with the three files.
