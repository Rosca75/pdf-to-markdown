# pdf_oxide evaluation (version 0.3.78)

Before building the engine, I checked whether `pdf_oxide` can provide the text structure the Markdown converter needs. The method was to read the source of the published crate and run a spike binary over a synthetic probe PDF that has headings, paragraphs, bullets, numbered items, italic and monospace text, an unruled table, a link annotation, a running header, page numbers, a word hyphenated across a page break, and two columns.

## What it provides (verified)

| Need | API | Result |
|---|---|---|
| Positioned text runs | `PdfDocument::extract_spans(page)` → `TextSpan` | One span per text run. Each span carries its text, bbox, `font_size`, `font_weight`, `is_italic`, `is_monospace`, `rotation_degrees` and `artifact_type`. This is exactly the input a layout analyser needs. |
| Bold / italic / monospace | `font_weight >= 600`, `is_italic`, `is_monospace` | Correct for the standard-14 fonts (Helvetica-Bold, Times-Italic, Courier). |
| Tagged or heuristic furniture | `artifact_type = Pagination(..)` | Bare page numbers were flagged even in an untagged file. Running headers were not; the engine handles them geometrically. |
| Links | `get_annotations(page)` → `action = LinkAction::Uri` + `rect` | Works. |
| Ruling lines (tables) | `extract_paths(page)` → `PathContent { bbox, stroke_color, .. }` | Available; the engine turns them into rules. |
| Encryption | `is_authenticated()`, `authenticate(password)` | The empty user password is tried automatically. `authenticate` returns `Ok(false)` for a wrong password. |
| Page geometry | `get_page_visible_box(page)` | CropBox clamped to MediaBox. Non-zero box origins are supported. |

## Limitations found

1. **It does not compile as published.** `pdf_oxide 0.3.78` declares `office_oxide = "0.1.9"`, but `office_oxide 0.1.13` added a required struct field, so a fresh resolve fails with `E0063: missing field defined_names`. Fix: pin `office_oxide = "=0.1.9"` in the workspace and commit `Cargo.lock`.
2. **The built-in Markdown converter is not good enough.** `to_markdown` with `detect_headings`, `extract_tables` and `strip_running_headers_footers` produced:
   - garbled tables (cells scattered between paragraphs);
   - `inter-national` (no dehyphenation);
   - no paragraph continuity across pages;
   - page numbers left in the output;
   - bold 14pt section titles emitted as `**bold**` instead of headings;
   - a link wrapped around a whole line.

   **Decision:** the engine uses only the extraction APIs above and runs its own layout pipeline (lines → furniture → tables → XY-cut reading order → segmentation → cross-page flow → classification → rendering).
3. **`PdfDocument` is neither `Send` nor `Sync`.** It holds `BufReader` state and interior caches. Parallel page extraction therefore needs one document per worker thread. See `docs/performance.md`.
4. **Memory.** `from_bytes` keeps two copies of the file (`source_bytes` plus the reader's buffer), so a document costs about 2× the file size before any parsing.
5. **Span bbox convention.** The `Rect` doc comment says "top-left corner", but the values are PDF user space with `y` at the bottom of the span (y up). The engine converts to a top-down frame in one place (`extract.rs`).
6. **Compile time.** About 450k lines of Rust. A clean release build takes about 15 minutes on the 2-core development laptop. The workspace builds pdf_oxide at `opt-level = 2` in dev and test profiles, so the corpus tests run at realistic speed.
7. **No OCR in default features.** This is out of scope anyway. Image-only pages yield no spans, and the engine reports `no_text_layer`.
