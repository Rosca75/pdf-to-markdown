import { percentOf, type State } from "../state";
import { plural, setAttr, setHidden, setText } from "./dom";
import type { Refs } from "./shell";

/** Characters of Markdown shown in the preview; Copy and Convert use it all. */
export const PREVIEW_LIMIT = 2_000_000;

const shown = new WeakMap<HTMLElement, string>();

/** Cut at a line boundary so the truncated preview ends cleanly. */
export function truncatePreview(text: string): { text: string; truncated: boolean } {
  if (text.length <= PREVIEW_LIMIT) return { text, truncated: false };
  const cut = text.lastIndexOf("\n", PREVIEW_LIMIT);
  return { text: text.slice(0, cut > 0 ? cut : PREVIEW_LIMIT), truncated: true };
}

interface Message {
  title: string;
  body: string;
  retry?: boolean;
  tone?: "error";
}

function messageFor(s: State): Message {
  const file = s.files.find((f) => f.id === s.selectedId);
  if (s.files.length === 0) {
    return {
      title: "Add PDFs to see their Markdown here",
      body: "Drop files on the left or click the drop area. Extraction starts right away and the result appears in this panel.",
    };
  }
  if (!file) {
    return { title: "Select a file", body: "Choose a file from the list to preview its Markdown." };
  }
  switch (file.status.kind) {
    case "queued":
      return { title: "Waiting to start", body: `${file.name} is next in line.` };
    case "extracting": {
      const p = percentOf(file);
      return {
        title: p === null ? "Extracting…" : `Extracting… ${p}%`,
        body: `The preview of ${file.name} appears when extraction finishes.`,
      };
    }
    case "password":
      return {
        title: "Password required",
        body: "Enter the password in the file list to unlock this PDF and preview it.",
      };
    case "error":
      return { title: file.status.message, body: file.status.hint, tone: "error" };
    case "ready":
      break;
  }
  const p = s.preview;
  if (p.kind === "error") {
    return { title: "The preview could not be loaded", body: p.message, retry: true, tone: "error" };
  }
  return { title: "Loading preview…", body: "" };
}

export function renderPreview(r: Refs, s: State): void {
  const file = s.files.find((f) => f.id === s.selectedId);
  const p = s.preview;
  const showText = !!file && p.kind === "ready" && p.id === file.id;

  setText(r.pvTitle, file ? file.name : "Preview");
  setAttr(r.pvTitle, "title", file ? file.path : null);

  const meta: string[] = [];
  let truncated = false;
  if (file && file.pages) meta.push(plural(file.pages, "page", "pages"));
  if (file && file.status.kind === "ready" && file.status.warnings.length > 0) {
    meta.push(plural(file.status.warnings.length, "warning", "warnings"));
    setAttr(r.pvMeta, "title", file.status.warnings.join("\n"));
  } else {
    setAttr(r.pvMeta, "title", null);
  }

  if (showText && p.kind === "ready") {
    const view = truncatePreview(p.text);
    truncated = view.truncated;
    // Set textContent once per document, never per line.
    if (shown.get(r.pvText) !== p.text) {
      r.pvText.textContent = view.text;
      r.pvText.scrollTop = 0;
      shown.set(r.pvText, p.text);
    }
  } else if (shown.has(r.pvText)) {
    r.pvText.textContent = "";
    shown.delete(r.pvText);
  }
  if (truncated) meta.push("preview shows the first part; Copy and Convert use the whole file");
  setText(r.pvMeta, meta.join(" · "));

  setHidden(r.pvText, !showText);
  setHidden(r.pvState, showText);
  if (!showText) {
    const m = messageFor(s);
    setText(r.pvStateTitle, m.title);
    setText(r.pvStateBody, m.body);
    setHidden(r.pvStateBody, m.body === "");
    r.pvState.dataset.tone = m.tone ?? "neutral";
    setHidden(r.pvRetry, !m.retry);
  }

  r.copy.disabled = !showText;
  setText(r.copy, s.copied ? "Copied" : "Copy");
  r.copy.classList.toggle("is-done", s.copied);
}
