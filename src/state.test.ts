import { describe, expect, it } from "vitest";
import {
  addPaths,
  canConvert,
  clearAll,
  createStore,
  extractFailed,
  fileName,
  finished,
  initialState,
  isPdfPath,
  overallProgress,
  percentOf,
  previewFailed,
  previewLoaded,
  progressed,
  removeFile,
  retryWithPassword,
  savingFinished,
  savingStarted,
  selectFile,
  togglePreviewMode,
  type State,
} from "./state";

function ids() {
  let n = 0;
  return () => `id${++n}`;
}

function withFiles(...paths: string[]): State {
  return addPaths(initialState(), paths, ids()).state;
}

const ok = (id: string, pages = 3, warnings: string[] = []) =>
  ({ id, ok: true, pages, warnings }) as const;

describe("path helpers", () => {
  it("extracts the file name from Windows and POSIX paths", () => {
    expect(fileName("C:\\docs\\a b.pdf")).toBe("a b.pdf");
    expect(fileName("/home/me/a.pdf")).toBe("a.pdf");
  });
  it("recognises PDFs case-insensitively", () => {
    expect(isPdfPath("a.PDF")).toBe(true);
    expect(isPdfPath("a.pdf.txt")).toBe(false);
    expect(isPdfPath("pdf")).toBe(false);
  });
});

describe("adding files", () => {
  it("adds PDFs as queued and selects the first", () => {
    const r = addPaths(initialState(), ["C:\\a.pdf", "C:\\b.pdf"], ids());
    expect(r.added.map((f) => f.name)).toEqual(["a.pdf", "b.pdf"]);
    expect(r.state.files.every((f) => f.status.kind === "queued")).toBe(true);
    expect(r.state.selectedId).toBe("id1");
  });

  it("keeps the current selection when more files arrive", () => {
    let s = withFiles("C:\\a.pdf");
    s = addPaths(s, ["C:\\b.pdf"], ids()).state;
    expect(s.selectedId).toBe("id1");
    expect(s.files).toHaveLength(2);
  });

  it("ignores duplicate paths, across calls and within one call", () => {
    let s = withFiles("C:\\a.pdf");
    const r = addPaths(s, ["C:\\a.pdf", "c:/A.PDF", "C:\\b.pdf", "C:\\b.pdf"], ids());
    s = r.state;
    expect(r.added).toHaveLength(1);
    expect(s.files.map((f) => f.name)).toEqual(["a.pdf", "b.pdf"]);
    expect(s.notice).toBeNull();
  });

  it("treats POSIX paths that differ in case as different files", () => {
    const s = withFiles("/x/A.pdf", "/x/a.pdf");
    expect(s.files).toHaveLength(2);
  });

  it("rejects non-PDFs with a notice naming them", () => {
    const r = addPaths(initialState(), ["C:\\a.pdf", "C:\\notes.docx", "C:\\x.txt"], ids());
    expect(r.rejected).toEqual(["notes.docx", "x.txt"]);
    expect(r.state.files).toHaveLength(1);
    expect(r.state.notice?.tone).toBe("warn");
    expect(r.state.notice?.text).toContain("notes.docx, x.txt");
    expect(r.state.notice?.text).toContain(".pdf");
  });

  it("uses singular wording and caps long name lists", () => {
    const one = addPaths(initialState(), ["a.txt"], ids()).state.notice?.text ?? "";
    expect(one).toContain("This file is not a PDF and was skipped");
    const many = addPaths(initialState(), ["1.txt", "2.txt", "3.txt", "4.txt", "5.txt"], ids());
    expect(many.state.notice?.text).toContain("1.txt, 2.txt, 3.txt and 2 more");
  });

  it("clears a stale notice when a clean add happens", () => {
    let s = addPaths(initialState(), ["x.txt"], ids()).state;
    expect(s.notice).not.toBeNull();
    s = addPaths(s, ["a.pdf"], ids()).state;
    expect(s.notice).toBeNull();
  });

  it("drops the previous save summary when files are added", () => {
    let s = withFiles("a.pdf");
    s = { ...s, result: { dir: "D", saved: [], failed: [] } };
    expect(addPaths(s, ["b.pdf"], ids()).state.result).toBeNull();
  });
});

describe("selection and removal", () => {
  it("select switches the file and asks for a preview only when ready", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = finished(s, ok("id2"));
    s = selectFile(s, "id2");
    expect(s.selectedId).toBe("id2");
    expect(s.preview.kind).toBe("loading");
    s = selectFile(s, "id1");
    expect(s.preview.kind).toBe("none");
  });

  it("selecting an unknown id changes nothing", () => {
    const s = withFiles("a.pdf");
    expect(selectFile(s, "nope")).toBe(s);
  });

  it("removing the selected file selects its neighbour", () => {
    let s = withFiles("a.pdf", "b.pdf", "c.pdf");
    s = selectFile(s, "id2");
    s = removeFile(s, "id2");
    expect(s.selectedId).toBe("id3");
    s = removeFile(s, "id3");
    expect(s.selectedId).toBe("id1");
    s = removeFile(s, "id1");
    expect(s.selectedId).toBeNull();
    expect(s.preview.kind).toBe("none");
  });

  it("removing another file keeps the selection", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = removeFile(s, "id2");
    expect(s.selectedId).toBe("id1");
  });

  it("clear all empties everything", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = clearAll(s);
    expect(s.files).toEqual([]);
    expect(s.selectedId).toBeNull();
  });

  it("the first file added after clearing becomes selected", () => {
    let s = clearAll(withFiles("a.pdf"));
    s = addPaths(s, ["z.pdf"], ids()).state;
    expect(s.selectedId).toBe(s.files[0].id);
  });
});

describe("progress", () => {
  it("moves queued to extracting and records the page count", () => {
    let s = withFiles("a.pdf");
    s = progressed(s, "id1", 0, 0);
    expect(s.files[0].status).toEqual({ kind: "extracting", done: 0, total: 0 });
    expect(s.files[0].pages).toBeNull();
    s = progressed(s, "id1", 5, 20);
    expect(s.files[0].pages).toBe(20);
    expect(percentOf(s.files[0])).toBe(25);
  });

  it("never moves backwards for the same total", () => {
    let s = withFiles("a.pdf");
    s = progressed(s, "id1", 10, 20);
    s = progressed(s, "id1", 8, 20);
    expect(s.files[0].status).toMatchObject({ done: 10 });
  });

  it("ignores progress for unknown or finished files", () => {
    let s = withFiles("a.pdf");
    expect(progressed(s, "zzz", 1, 2)).toBe(s);
    s = finished(s, ok("id1"));
    expect(progressed(s, "id1", 1, 3)).toBe(s);
  });

  it("aggregates pages across files", () => {
    let s = withFiles("a.pdf", "b.pdf", "c.pdf");
    s = progressed(s, "id1", 10, 40);
    s = progressed(s, "id2", 0, 0);
    s = finished(s, ok("id3", 60));
    const o = overallProgress(s);
    expect(o).toMatchObject({ active: 2, ready: 1, tracked: 3, pagesDone: 70, pagesTotal: 100, percent: 70 });
  });

  it("excludes password and failed files from the aggregate", () => {
    let s = withFiles("a.pdf", "b.pdf", "c.pdf");
    s = finished(s, { id: "id1", ok: false, error: { code: "password_required", message: "m", hint: "h" } });
    s = finished(s, { id: "id2", ok: false, error: { code: "corrupt", message: "m", hint: "h" } });
    s = finished(s, ok("id3", 4));
    const o = overallProgress(s);
    expect(o).toMatchObject({ active: 0, tracked: 1, pagesDone: 4, pagesTotal: 4, percent: 100 });
  });

  it("reports no percent before any page total is known", () => {
    const s = withFiles("a.pdf");
    expect(overallProgress(s).percent).toBeNull();
  });
});

describe("completion", () => {
  it("marks a file ready with its page count and warnings", () => {
    let s = withFiles("a.pdf");
    s = finished(s, ok("id1", 7, ["Page 2: skipped"]));
    expect(s.files[0].pages).toBe(7);
    expect(s.files[0].status).toEqual({ kind: "ready", warnings: ["Page 2: skipped"] });
  });

  it("requests a preview when the selected file becomes ready", () => {
    let s = withFiles("a.pdf");
    s = finished(s, ok("id1"));
    expect(s.preview).toMatchObject({ kind: "loading", id: "id1" });
  });

  it("does not request a preview for a file that is not selected", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = finished(s, ok("id2"));
    expect(s.preview.kind).toBe("none");
  });

  it("keeps error code, message and hint", () => {
    let s = withFiles("a.pdf");
    s = finished(s, {
      id: "id1",
      ok: false,
      error: { code: "no_text_layer", message: "No text.", hint: "Run OCR first." },
    });
    expect(s.files[0].status).toEqual({
      kind: "error",
      code: "no_text_layer",
      message: "No text.",
      hint: "Run OCR first.",
    });
  });

  it("ignores events for removed files", () => {
    const s = withFiles("a.pdf");
    expect(finished(s, ok("ghost"))).toBe(s);
  });

  it("supplies a message when the backend sends none", () => {
    const s = finished(withFiles("a.pdf"), { id: "id1", ok: false });
    expect(s.files[0].status).toMatchObject({ kind: "error" });
  });

  it("extractFailed turns an IPC failure into an actionable error", () => {
    const s = extractFailed(withFiles("a.pdf"), "id1", "boom");
    const st = s.files[0].status;
    expect(st.kind).toBe("error");
    if (st.kind === "error") {
      expect(st.message).toBe("boom");
      expect(st.hint).not.toBe("");
    }
  });
});

describe("password flow", () => {
  const needPw = { code: "password_required", message: "m", hint: "h" };

  it("password_required parks the file awaiting a password", () => {
    const s = finished(withFiles("a.pdf"), { id: "id1", ok: false, error: needPw });
    expect(s.files[0].status).toEqual({ kind: "password", wrong: false });
  });

  it("retrying goes back to extracting", () => {
    let s = finished(withFiles("a.pdf"), { id: "id1", ok: false, error: needPw });
    s = retryWithPassword(s, "id1");
    expect(s.files[0].status.kind).toBe("extracting");
  });

  it("wrong_password flags the retry as wrong", () => {
    let s = finished(withFiles("a.pdf"), { id: "id1", ok: false, error: needPw });
    s = retryWithPassword(s, "id1");
    s = finished(s, { id: "id1", ok: false, error: { code: "wrong_password", message: "m", hint: "h" } });
    expect(s.files[0].status).toEqual({ kind: "password", wrong: true });
  });

  it("a correct password ends in ready", () => {
    let s = finished(withFiles("a.pdf"), { id: "id1", ok: false, error: needPw });
    s = retryWithPassword(s, "id1");
    s = finished(s, ok("id1", 9));
    expect(s.files[0].status.kind).toBe("ready");
  });

  it("retry is ignored unless a password is awaited", () => {
    const s = withFiles("a.pdf");
    expect(retryWithPassword(s, "id1")).toBe(s);
  });
});

describe("convert-enabled rule", () => {
  it("is off with no files and while nothing is ready", () => {
    expect(canConvert(initialState())).toBe(false);
    let s = withFiles("a.pdf");
    expect(canConvert(s)).toBe(false);
    s = progressed(s, "id1", 1, 4);
    expect(canConvert(s)).toBe(false);
  });

  it("turns on as soon as one file is ready, even while others extract", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = progressed(s, "id1", 1, 4);
    s = finished(s, ok("id2"));
    expect(canConvert(s)).toBe(true);
  });

  it("stays off for password-required and failed files", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = finished(s, { id: "id1", ok: false, error: { code: "password_required", message: "", hint: "" } });
    s = finished(s, { id: "id2", ok: false, error: { code: "corrupt", message: "", hint: "" } });
    expect(canConvert(s)).toBe(false);
  });

  it("is off while saving and back on afterwards", () => {
    let s = finished(withFiles("a.pdf"), ok("id1"));
    s = savingStarted(s);
    expect(canConvert(s)).toBe(false);
    s = savingFinished(s, { dir: "D", saved: [], failed: [] });
    expect(canConvert(s)).toBe(true);
  });
});

describe("preview results", () => {
  it("stores loaded text for the matching request only", () => {
    let s = finished(withFiles("a.pdf"), ok("id1"));
    const p = s.preview;
    if (p.kind !== "loading") throw new Error("expected loading");
    expect(previewLoaded(s, "id1", p.seq + 1, "stale")).toBe(s);
    s = previewLoaded(s, "id1", p.seq, "# Title");
    expect(s.preview).toEqual({ kind: "ready", id: "id1", text: "# Title" });
  });

  it("drops a late answer after the selection moved", () => {
    let s = withFiles("a.pdf", "b.pdf");
    s = finished(s, ok("id1"));
    const p = s.preview;
    if (p.kind !== "loading") throw new Error("expected loading");
    s = selectFile(s, "id2");
    expect(previewLoaded(s, "id1", p.seq, "late")).toBe(s);
  });

  it("records failures", () => {
    let s = finished(withFiles("a.pdf"), ok("id1"));
    const p = s.preview;
    if (p.kind !== "loading") throw new Error("expected loading");
    s = previewFailed(s, "id1", p.seq, "gone");
    expect(s.preview).toEqual({ kind: "error", id: "id1", message: "gone" });
  });
});

describe("store", () => {
  it("notifies subscribers only on change and supports unsubscribe", () => {
    const store = createStore();
    let calls = 0;
    const off = store.subscribe(() => calls++);
    store.update((s) => s);
    expect(calls).toBe(0);
    store.update((s) => addPaths(s, ["a.pdf"], ids()).state);
    expect(calls).toBe(1);
    off();
    store.update((s) => addPaths(s, ["b.pdf"], ids()).state);
    expect(calls).toBe(1);
  });
});

describe("preview mode", () => {
  it("starts on the rendered view and toggles both ways", () => {
    const s = initialState();
    expect(s.previewMode).toBe("markdown");
    const raw = togglePreviewMode(s);
    expect(raw.previewMode).toBe("raw");
    expect(togglePreviewMode(raw).previewMode).toBe("markdown");
  });

  it("keeps the chosen mode when the selection changes", () => {
    let s = togglePreviewMode(withFiles("C:\\a.pdf", "C:\\b.pdf"));
    s = selectFile(s, "id2");
    expect(s.previewMode).toBe("raw");
  });
});
