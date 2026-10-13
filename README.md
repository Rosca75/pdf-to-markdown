# PDF2MD

A small, fast desktop app that converts PDF files to Markdown. Drop in one or many PDFs, preview the Markdown, and save the results as `.md` files. Everything runs locally on your machine; nothing is uploaded anywhere.

The conversion engine is written in Rust, the shell is [Tauri v2](https://tauri.app), and the UI is plain TypeScript built with Vite.

## Download

Grab the latest build from the [Releases page](https://github.com/Rosca75/pdf-to-markdown/releases/latest). All builds are portable: no installer, no admin rights.

| Platform | File | Notes |
|---|---|---|
| Windows 11 (x64) | `PDF2MD_<version>_windows_x64_portable.exe` | Needs WebView2 (included with Windows 11) |
| Linux (x86_64, Ubuntu 24.04) | `PDF2MD_<version>_linux_x86_64.AppImage` | May need `libfuse2t64` |
| macOS (Apple Silicon and Intel) | `PDF2MD_<version>_macos_universal.zip` | Contains `PDF2MD.app` |

Each release also includes `SHA256SUMS.txt` so you can verify your download.

### First run (the binaries are unsigned)

The builds are not code-signed, so your OS will warn you the first time. This is expected.

**Windows.** SmartScreen shows "Windows protected your PC". Click **More info**, then **Run anyway**. The app needs the Microsoft Edge WebView2 runtime, which ships with Windows 11. On older systems, install it from Microsoft.

**macOS.** Unzip the download, then either right-click `PDF2MD.app` and choose **Open** (confirm in the dialog), or remove the quarantine flag once from a terminal:

```sh
xattr -cr PDF2MD.app
```

**Linux.** Make the AppImage executable and run it:

```sh
chmod +x PDF2MD_*_linux_x86_64.AppImage
./PDF2MD_*_linux_x86_64.AppImage
```

On Ubuntu 24.04, AppImages need FUSE 2, which is not installed by default:

```sh
sudo apt install libfuse2t64
```

## Using it

1. Add PDFs by dragging them onto the window or clicking the drop zone.
2. Each file is extracted in the background; the progress bar tracks it. Click a file to preview its Markdown source (read-only, with a Copy button).
3. Click **Convert to Markdown** and pick an output folder. The app remembers the last folder you used.

Existing files are never overwritten: if `report.md` already exists, the new file is saved as `report (1).md`, then `report (2).md`, and so on. If one file in a batch fails, the others still convert, and the failed file shows what went wrong. Password-protected PDFs open silently if the password is empty; otherwise the file asks for the password inline.

## Features

- Batch conversion of many PDFs at once, with drag and drop.
- Headings, paragraphs, lists, bold and italic, tables, and hyperlinks.
- Repeated running headers, footers, and page numbers are stripped.
- Text flows seamlessly across pages, and words hyphenated across a page break are rejoined. There are no page markers.
- Password-protected PDFs are supported.
- Follows the OS light or dark theme; keyboard accessible.

## Limitations

- **No OCR.** Scanned documents with no text layer cannot be converted; the app tells you so.
- **Images, shapes, and diagrams are dropped.** Only text content reaches the Markdown.
- **Tables are best-effort.** Detection works well for ruled and cleanly aligned tables and can miss or mis-split complex layouts (merged cells, nested tables).
- Heading and list detection is heuristic, based on font size, weight, and layout, so unusual documents may need touch-ups.

## Building from source

Prerequisites:

- [Rust](https://rustup.rs) 1.88 or newer (stable)
- Node.js LTS (22.x or 24.x) and npm
- Linux only: the Tauri system libraries.

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
    patchelf libxdo-dev libssl-dev build-essential curl wget file
  ```

- Windows: WebView2 (preinstalled on Windows 11) and the MSVC build tools.

```sh
git clone https://github.com/Rosca75/pdf-to-markdown.git
cd pdf-to-markdown
npm ci

npm run tauri -- dev                      # run the app with hot reload
npm run tauri -- build --no-bundle        # release binary only (target/release/pdf2md)
npm run tauri -- build --bundles appimage # Linux AppImage
npm run tauri -- build --target universal-apple-darwin --bundles app  # macOS universal .app
```

The Rust crates compile against the built frontend, so run `npm run build` (which produces `dist/`) before plain `cargo build`, `cargo clippy`, or `cargo test` on the workspace.

## Running the tests

```sh
npm ci
npm run build                      # the Tauri crate needs dist/ to exist
npm test                           # frontend unit tests (Vitest)
cargo test --workspace --release   # engine, fixtures, and shell tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Release mode is used for Rust tests because the corpus integration tests convert a set of committed PDFs in `tests/corpus/` and are slow in debug builds. CI runs all of the above on Ubuntu, Windows, and macOS for every push to `main` and every pull request.

## Project layout

```
Cargo.toml                 Cargo workspace
crates/pdf2md-engine/      conversion engine (library plus a `pdf2md` CLI for corpus runs and benches)
crates/pdf-fixtures/       dev-only PDF writer and corpus generator
tests/corpus/              generated, committed corpus PDFs and expectations
src-tauri/                 Tauri v2 app shell (commands, events, state)
src/                       frontend (TypeScript + Vite, no framework)
docs/                      design spec, plans, and reports
.github/workflows/         ci.yml (tests on push/PR), release.yml (tag v*)
```

## Releasing

Push a tag like `v0.1.0`. The release workflow runs the tests, builds the three artifacts on their native runners, and publishes a GitHub Release with the files and `SHA256SUMS.txt`.

## License

[MIT](LICENSE)
