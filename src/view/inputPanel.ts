import { overallProgress, type State } from "../state";
import { plural, setAttr, setHidden, setText } from "./dom";
import type { Refs } from "./shell";

/** Drop zone, notice strip, overall progress and list header. */
export function renderInputPanel(r: Refs, s: State): void {
  const empty = s.files.length === 0;

  r.dropzone.classList.toggle("is-compact", !empty);
  r.dropzone.classList.toggle("is-dragging", s.dragging);
  if (s.dragging) {
    setText(r.dropTitle, "Release to add the files");
    setText(r.dropSub, "Only .pdf files are accepted");
  } else if (empty) {
    setText(r.dropTitle, "Drop PDF files here");
    setText(r.dropSub, "or click to browse");
  } else {
    setText(r.dropTitle, "Add more PDFs");
    setText(r.dropSub, "Drop files here or click to browse");
  }

  // Notice
  setHidden(r.notice, s.notice === null);
  r.notice.dataset.tone = s.notice?.tone ?? "info";
  setText(r.noticeText, s.notice?.text ?? "");

  // Overall progress
  const o = overallProgress(s);
  setHidden(r.progress, o.tracked === 0);
  if (o.tracked > 0) {
    const pct = o.active === 0 ? 100 : o.percent;
    const files = plural(o.tracked, "file", "files");
    let text: string;
    if (o.active === 0) {
      text = `${files} extracted, ${plural(o.pagesTotal, "page", "pages")}`;
    } else if (o.pagesTotal > 0) {
      text = `Extracting ${files}: ${o.pagesDone} of ${o.pagesTotal} pages (${pct}%)`;
    } else {
      text = `Starting ${files}…`;
    }
    setText(r.progressText, text);
    r.progress.classList.toggle("is-indeterminate", pct === null);
    r.progress.classList.toggle("is-complete", o.active === 0);
    if (pct === null) {
      setAttr(r.progressBar, "aria-valuenow", null);
      r.progressFill.style.width = "";
    } else {
      setAttr(r.progressBar, "aria-valuenow", String(pct));
      const w = `${pct}%`;
      if (r.progressFill.style.width !== w) r.progressFill.style.width = w;
    }
    setAttr(r.progressBar, "aria-valuetext", text);
  }

  // List header
  setHidden(r.listHead, empty);
  setText(r.listCount, plural(s.files.length, "file", "files"));
}
