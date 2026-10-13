import { afterEach, describe, expect, it, vi } from "vitest";
import type { Api, DragEvent, DonePayload, ProgressPayload, SaveOutcome } from "./api";
import { mount, summarize, type App } from "./app";
import { PREVIEW_LIMIT, truncatePreview } from "./view/preview";

/* ---------- harness ---------- */

interface Harness {
  app: App;
  root: HTMLElement;
  api: ReturnType<typeof makeApi>["api"];
  store: Map<string, string>;
  copy: ReturnType<typeof vi.fn>;
  progress(p: ProgressPayload): void;
  done(d: DonePayload): void;
  drag(e: DragEvent): void;
}

function makeApi() {
  const handlers: {
    progress?: (p: ProgressPayload) => void;
    done?: (d: DonePayload) => void;
    drag?: (e: DragEvent) => void;
  } = {};
  const api = {
    pickPdfs: vi.fn<Api["pickPdfs"]>(async () => []),
    pickFolder: vi.fn<Api["pickFolder"]>(async () => null),
    extract: vi.fn<Api["extract"]>(async () => undefined),
    getMarkdown: vi.fn<Api["getMarkdown"]>(async (id) => `# Markdown of ${id}`),
    remove: vi.fn<Api["remove"]>(async () => undefined),
    saveAll: vi.fn<Api["saveAll"]>(async (_dir, items) =>
      items.map((i): SaveOutcome => ({ id: i.id, path: `OUT\\${i.name.replace(/\.pdf$/i, ".md")}` })),
    ),
    onProgress: vi.fn<Api["onProgress"]>(async (cb) => {
      handlers.progress = cb;
      return () => undefined;
    }),
    onDone: vi.fn<Api["onDone"]>(async (cb) => {
      handlers.done = cb;
      return () => undefined;
    }),
    onDragDrop: vi.fn<Api["onDragDrop"]>(async (cb) => {
      handlers.drag = cb;
      return () => undefined;
    }),
  };
  return { api, handlers };
}

const flush = () => new Promise<void>((r) => setTimeout(r, 0));

let current: Harness | null = null;

async function setup(): Promise<Harness> {
  const root = document.createElement("div");
  document.body.append(root);
  const { api, handlers } = makeApi();
  const data = new Map<string, string>();
  const copy = vi.fn(async () => undefined);
  let n = 0;
  const app = await mount(root, {
    api,
    storage: { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) },
    newId: () => `id${++n}`,
    copy,
  });
  const h: Harness = {
    app,
    root,
    api,
    store: data,
    copy,
    progress: (p) => handlers.progress?.(p),
    done: (d) => handlers.done?.(d),
    drag: (e) => handlers.drag?.(e),
  };
  current = h;
  return h;
}

afterEach(() => {
  current?.app.dispose();
  current?.root.remove();
  current = null;
  vi.restoreAllMocks();
});

const q = <T extends Element = HTMLElement>(h: Harness, sel: string): T => {
  const found = h.root.querySelector<T>(sel);
  if (!found) throw new Error(`no element for ${sel}`);
  return found;
};
const qa = (h: Harness, sel: string) => [...h.root.querySelectorAll<HTMLElement>(sel)];
const text = (h: Harness, sel: string) => q(h, sel).textContent?.trim() ?? "";
const apiOf = (h: Harness) => h.api;

const convertBtn = (h: Harness) => q<HTMLButtonElement>(h, "#convert");
const rows = (h: Harness) => qa(h, "#files > li");

async function addViaDialog(h: Harness, ...paths: string[]) {
  apiOf(h).pickPdfs.mockResolvedValueOnce(paths);
  q(h, "#dropzone").click();
  await flush();
}

const okDone = (id: string, pages = 3): DonePayload => ({ id, ok: true, pages, warnings: [] });

/* ---------- tests ---------- */

describe("welcome state", () => {
  it("shows an empty preview, a disabled Convert button and a focusable drop zone", async () => {
    const h = await setup();
    expect(convertBtn(h).disabled).toBe(true);
    expect(convertBtn(h).textContent).toBe("Convert to Markdown");
    expect(text(h, "#bar-status")).toBe("No files yet");
    expect(text(h, "#pv-state-title")).toContain("Add PDFs");
    expect(q(h, "#pv-text").hidden).toBe(true);
    expect(q<HTMLButtonElement>(h, "#copy").disabled).toBe(true);
    expect(q(h, "#progress").hidden).toBe(true);
    expect(q(h, "#list-head").hidden).toBe(true);
    const dz = q(h, "#dropzone");
    expect(dz.getAttribute("tabindex")).toBe("0");
    expect(dz.getAttribute("role")).toBe("button");
    expect(dz.classList.contains("is-compact")).toBe(false);
  });
});

describe("adding files", () => {
  it("opens the native dialog on click and starts extraction for each file", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\docs\\a.pdf", "C:\\docs\\b.pdf");
    expect(apiOf(h).pickPdfs).toHaveBeenCalledTimes(1);
    expect(apiOf(h).extract).toHaveBeenCalledTimes(2);
    expect(apiOf(h).extract).toHaveBeenCalledWith("id1", "C:\\docs\\a.pdf", undefined);
    expect(apiOf(h).extract).toHaveBeenCalledWith("id2", "C:\\docs\\b.pdf", undefined);
    expect(rows(h).map((li) => li.querySelector(".file-name")?.textContent)).toEqual(["a.pdf", "b.pdf"]);
    expect(rows(h).map((li) => li.querySelector(".chip")?.textContent)).toEqual(["Queued", "Queued"]);
    expect(q(h, "#dropzone").classList.contains("is-compact")).toBe(true);
  });

  it.each(["Enter", " "])("opens the dialog with the %j key", async (key) => {
    const h = await setup();
    const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
    q(h, "#dropzone").dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    expect(apiOf(h).pickPdfs).toHaveBeenCalledTimes(1);
  });

  it("does nothing when the dialog is cancelled", async () => {
    const h = await setup();
    await addViaDialog(h);
    expect(rows(h)).toHaveLength(0);
    expect(apiOf(h).extract).not.toHaveBeenCalled();
  });

  it("ignores a path that is already in the list", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    expect(rows(h)).toHaveLength(2);
    expect(apiOf(h).extract).toHaveBeenCalledTimes(2);
  });

  it("shows an actionable message when the dialog fails", async () => {
    const h = await setup();
    apiOf(h).pickPdfs.mockRejectedValueOnce("denied");
    q(h, "#dropzone").click();
    await flush();
    expect(q(h, "#notice").hidden).toBe(false);
    expect(text(h, "#notice-text")).toContain("denied");
  });

  it("marks a file as failed when the backend refuses to start", async () => {
    const h = await setup();
    apiOf(h).extract.mockRejectedValueOnce("ipc down");
    await addViaDialog(h, "C:\\a.pdf");
    await flush();
    expect(rows(h)[0].dataset.status).toBe("error");
    expect(rows(h)[0].textContent).toContain("ipc down");
  });
});

describe("drag and drop", () => {
  it("adds dropped PDFs and rejects the rest by name", async () => {
    const h = await setup();
    h.drag({ type: "drop", paths: ["C:\\x\\a.pdf", "C:\\x\\notes.docx", "C:\\x\\pic.png"] });
    expect(apiOf(h).extract).toHaveBeenCalledTimes(1);
    expect(rows(h)).toHaveLength(1);
    expect(q(h, "#notice").hidden).toBe(false);
    expect(text(h, "#notice-text")).toContain("notes.docx, pic.png");
  });

  it("dropping only non-PDFs starts nothing", async () => {
    const h = await setup();
    h.drag({ type: "drop", paths: ["C:\\a.txt"] });
    expect(apiOf(h).extract).not.toHaveBeenCalled();
    expect(text(h, "#notice-text")).toContain("a.txt");
  });

  it("highlights the drop zone while dragging and clears on leave and drop", async () => {
    const h = await setup();
    const dz = q(h, "#dropzone");
    h.drag({ type: "enter", paths: ["C:\\a.pdf"] });
    expect(dz.classList.contains("is-dragging")).toBe(true);
    h.drag({ type: "leave" });
    expect(dz.classList.contains("is-dragging")).toBe(false);
    h.drag({ type: "enter", paths: [] });
    h.drag({ type: "drop", paths: ["C:\\a.pdf"] });
    expect(dz.classList.contains("is-dragging")).toBe(false);
  });

  it("the notice can be dismissed", async () => {
    const h = await setup();
    h.drag({ type: "drop", paths: ["C:\\a.txt"] });
    q(h, "#notice-dismiss").click();
    expect(q(h, "#notice").hidden).toBe(true);
  });
});

describe("statuses and progress", () => {
  it("shows extracting percentage per file and overall progress", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    h.progress({ id: "id1", done: 5, total: 10 });
    h.progress({ id: "id2", done: 0, total: 0 });
    expect(rows(h)[0].querySelector(".chip")?.textContent).toBe("Extracting 50%");
    expect(rows(h)[1].querySelector(".chip")?.textContent).toBe("Extracting");
    const bar = rows(h)[0].querySelector(".file-extra [role=progressbar]");
    expect(bar?.getAttribute("aria-valuenow")).toBe("50");
    expect(q(h, "#progress").hidden).toBe(false);
    expect(text(h, "#progress-text")).toContain("5 of 10 pages (50%)");
    expect(q(h, "#progress-bar").getAttribute("aria-valuenow")).toBe("50");
    expect(rows(h)[0].querySelector(".file-meta")?.textContent).toBe("10 pages");
  });

  it("turns a file Ready and completes the overall bar", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.progress({ id: "id1", done: 2, total: 4 });
    h.done(okDone("id1", 4));
    await flush();
    expect(rows(h)[0].querySelector(".chip")?.textContent).toBe("Ready");
    expect(text(h, "#progress-text")).toContain("1 file extracted, 4 pages");
    expect(q(h, "#progress-bar").getAttribute("aria-valuenow")).toBe("100");
    expect(text(h, "#bar-status")).toBe("1 of 1 file ready");
  });

  it("shows error message and hint from the backend", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done({
      id: "id1",
      ok: false,
      error: { code: "no_text_layer", message: "No text found.", hint: "Run OCR first." },
    });
    const li = rows(h)[0];
    expect(li.dataset.status).toBe("error");
    expect(li.querySelector(".chip")?.textContent).toBe("Failed");
    expect(li.querySelector(".file-error")?.textContent).toBe("No text found.");
    expect(li.querySelector(".file-hint")?.textContent).toBe("Run OCR first.");
    expect(text(h, "#pv-state-title")).toBe("No text found.");
    expect(text(h, "#pv-state-body")).toBe("Run OCR first.");
  });

  it("lists warnings in the meta line and preview header", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done({ id: "id1", ok: true, pages: 2, warnings: ["Page 2: skipped"] });
    expect(rows(h)[0].querySelector(".file-meta")?.textContent).toBe("2 pages · 1 warning");
    expect(q(h, "#pv-meta").getAttribute("title")).toBe("Page 2: skipped");
  });

  it("keeps the row element and its focus across progress updates", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    const main = rows(h)[0].querySelector<HTMLButtonElement>(".file-main");
    main?.focus();
    h.progress({ id: "id1", done: 1, total: 10 });
    h.progress({ id: "id1", done: 2, total: 10 });
    expect(rows(h)[0].querySelector(".file-main")).toBe(main);
    expect(document.activeElement).toBe(main);
  });
});

describe("password flow", () => {
  const needPassword: DonePayload = {
    id: "id1",
    ok: false,
    error: { code: "password_required", message: "m", hint: "h" },
  };

  async function locked() {
    const h = await setup();
    await addViaDialog(h, "C:\\secret.pdf");
    h.done(needPassword);
    return h;
  }
  const form = (h: Harness) => q<HTMLFormElement>(h, "#files form");
  const submit = (h: Harness) =>
    form(h).dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

  it("shows an inline password field and an Unlock button", async () => {
    const h = await locked();
    expect(rows(h)[0].querySelector(".chip")?.textContent).toBe("Password required");
    expect(form(h).querySelector("input")?.type).toBe("password");
    expect(form(h).querySelector("button")?.textContent).toBe("Unlock");
  });

  it("re-runs extraction with the typed password", async () => {
    const h = await locked();
    form(h).querySelector("input")!.value = "s3cret";
    apiOf(h).extract.mockClear();
    submit(h);
    expect(apiOf(h).extract).toHaveBeenCalledWith("id1", "C:\\secret.pdf", "s3cret");
    expect(rows(h)[0].dataset.status).toBe("extracting");
    expect(h.root.querySelector("#files form")).toBeNull();
  });

  it("does not call the backend for an empty password", async () => {
    const h = await locked();
    apiOf(h).extract.mockClear();
    submit(h);
    expect(apiOf(h).extract).not.toHaveBeenCalled();
    expect(rows(h)[0].dataset.status).toBe("password");
  });

  it("says so when the password was wrong and puts focus back in the field", async () => {
    const h = await locked();
    form(h).querySelector("input")!.value = "nope";
    submit(h);
    h.done({ id: "id1", ok: false, error: { code: "wrong_password", message: "m", hint: "h" } });
    expect(rows(h)[0].querySelector(".file-note")?.textContent).toContain("incorrect");
    expect(document.activeElement).toBe(form(h).querySelector("input"));
  });

  it("keeps the typed text when unrelated files change", async () => {
    const h = await locked();
    await addViaDialog(h, "C:\\other.pdf");
    const input = form(h).querySelector("input")!;
    input.value = "half-typed";
    h.progress({ id: "id2", done: 1, total: 5 });
    expect(form(h).querySelector("input")).toBe(input);
    expect(input.value).toBe("half-typed");
  });

  it("becomes ready after the right password", async () => {
    const h = await locked();
    form(h).querySelector("input")!.value = "ok";
    submit(h);
    h.done(okDone("id1"));
    expect(rows(h)[0].dataset.status).toBe("ready");
    expect(convertBtn(h).disabled).toBe(false);
  });
});

describe("selection and preview", () => {
  it("previews the first loaded file by default", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    h.done(okDone("id2"));
    h.done(okDone("id1"));
    await flush();
    expect(rows(h)[0].classList.contains("is-selected")).toBe(true);
    expect(apiOf(h).getMarkdown).toHaveBeenCalledTimes(1);
    expect(apiOf(h).getMarkdown).toHaveBeenCalledWith("id1");
    expect(q(h, "#pv-text").hidden).toBe(false);
    expect(q(h, "#pv-text").textContent).toBe("# Markdown of id1");
    expect(text(h, "#pv-title")).toBe("a.pdf");
    expect(q<HTMLButtonElement>(h, "#copy").disabled).toBe(false);
  });

  it("clicking another file selects and previews it", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    h.done(okDone("id1"));
    h.done(okDone("id2"));
    await flush();
    rows(h)[1].querySelector<HTMLButtonElement>(".file-main")!.click();
    expect(rows(h)[1].classList.contains("is-selected")).toBe(true);
    expect(rows(h)[1].querySelector(".file-main")?.getAttribute("aria-current")).toBe("true");
    expect(rows(h)[0].querySelector(".file-main")?.getAttribute("aria-current")).toBeNull();
    expect(q(h, "#pv-text").hidden).toBe(true);
    expect(text(h, "#pv-state-title")).toBe("Loading preview…");
    await flush();
    expect(q(h, "#pv-text").textContent).toBe("# Markdown of id2");
    expect(text(h, "#pv-title")).toBe("b.pdf");
  });

  it("explains why a file that is not ready has no preview", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.progress({ id: "id1", done: 3, total: 12 });
    expect(text(h, "#pv-state-title")).toBe("Extracting… 25%");
    h.done({ id: "id1", ok: false, error: { code: "password_required", message: "m", hint: "h" } });
    expect(text(h, "#pv-state-title")).toBe("Password required");
  });

  it("ignores a late preview for a file that is no longer selected", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    let release: (v: string) => void = () => undefined;
    apiOf(h).getMarkdown.mockImplementationOnce(() => new Promise<string>((r) => (release = r)));
    h.done(okDone("id1"));
    h.done(okDone("id2"));
    rows(h)[1].querySelector<HTMLButtonElement>(".file-main")!.click();
    await flush();
    release("# STALE");
    await flush();
    expect(q(h, "#pv-text").textContent).toBe("# Markdown of id2");
  });

  it("shows an error with a retry when the preview cannot be fetched", async () => {
    const h = await setup();
    apiOf(h).getMarkdown.mockRejectedValueOnce("gone");
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    await flush();
    expect(text(h, "#pv-state-title")).toBe("The preview could not be loaded");
    expect(text(h, "#pv-state-body")).toBe("gone");
    const retry = q<HTMLButtonElement>(h, "#pv-retry");
    expect(retry.hidden).toBe(false);
    retry.click();
    await flush();
    expect(q(h, "#pv-text").textContent).toBe("# Markdown of id1");
  });

  it("renders large output as one text node, not a node per line", async () => {
    const h = await setup();
    const big = Array.from({ length: 50_000 }, (_, i) => `line ${i}`).join("\n");
    apiOf(h).getMarkdown.mockResolvedValueOnce(big);
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    await flush();
    const pre = q(h, "#pv-text");
    expect(pre.childNodes).toHaveLength(1);
    expect(pre.childElementCount).toBe(0);
    expect(pre.textContent).toBe(big);
  });

  it("truncates only the display beyond the preview limit", () => {
    const small = truncatePreview("abc");
    expect(small).toEqual({ text: "abc", truncated: false });
    const huge = ("x".repeat(99) + "\n").repeat(Math.ceil(PREVIEW_LIMIT / 100) + 10);
    const t = truncatePreview(huge);
    expect(t.truncated).toBe(true);
    expect(t.text.length).toBeLessThanOrEqual(PREVIEW_LIMIT);
    expect(t.text.endsWith("x")).toBe(true);
  });

  it("copies the full Markdown and confirms", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    await flush();
    q(h, "#copy").click();
    await flush();
    expect(h.copy).toHaveBeenCalledWith("# Markdown of id1");
    expect(text(h, "#copy")).toBe("Copied");
  });

  it("reports a failed copy", async () => {
    const h = await setup();
    h.copy.mockRejectedValueOnce(new Error("blocked"));
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    await flush();
    q(h, "#copy").click();
    await flush();
    expect(text(h, "#copy")).toBe("Copy");
    expect(text(h, "#notice-text")).toContain("Copy failed");
  });

  it("preview is a read-only monospace region that can be focused", async () => {
    const h = await setup();
    const pre = q(h, "#pv-text");
    expect(pre.tagName).toBe("PRE");
    expect(pre.getAttribute("tabindex")).toBe("0");
    expect(pre.hasAttribute("contenteditable")).toBe(false);
  });
});

describe("removing files", () => {
  it("remove tells the backend, drops the row and moves the selection", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    h.done(okDone("id2"));
    q<HTMLButtonElement>(h, "li[data-id=id1] .file-remove").click();
    await flush();
    expect(apiOf(h).remove).toHaveBeenCalledWith("id1");
    expect(rows(h)).toHaveLength(1);
    expect(rows(h)[0].classList.contains("is-selected")).toBe(true);
    expect(q(h, "#pv-text").textContent).toBe("# Markdown of id2");
  });

  it("remove buttons name their file for screen readers", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    expect(q(h, ".file-remove").getAttribute("aria-label")).toBe("Remove a.pdf");
  });

  it("Clear all removes every file and resets the panels", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    h.done(okDone("id1"));
    await flush();
    q(h, "#clear-all").click();
    expect(apiOf(h).remove).toHaveBeenCalledTimes(2);
    expect(rows(h)).toHaveLength(0);
    expect(q(h, "#pv-text").hidden).toBe(true);
    expect(q(h, "#pv-text").textContent).toBe("");
    expect(convertBtn(h).disabled).toBe(true);
    expect(q(h, "#list-head").hidden).toBe(true);
  });

  it("events for removed files are ignored", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    q<HTMLButtonElement>(h, ".file-remove").click();
    h.done(okDone("id1"));
    h.progress({ id: "id1", done: 1, total: 2 });
    expect(rows(h)).toHaveLength(0);
  });
});

describe("Convert to Markdown", () => {
  it("is disabled until a file is ready and explains why", async () => {
    const h = await setup();
    expect(convertBtn(h).title).toContain("Add a PDF");
    await addViaDialog(h, "C:\\a.pdf");
    expect(convertBtn(h).disabled).toBe(true);
    expect(convertBtn(h).title).toContain("finish extracting");
    h.done(okDone("id1"));
    expect(convertBtn(h).disabled).toBe(false);
    expect(convertBtn(h).title).toBe("");
  });

  it("asks for a folder, remembers it and saves only the ready files", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\in\\a.pdf", "C:\\in\\b.pdf", "C:\\in\\c.pdf");
    h.done(okDone("id1"));
    h.done(okDone("id3"));
    apiOf(h).pickFolder.mockResolvedValueOnce("C:\\out");
    convertBtn(h).click();
    await flush();
    expect(apiOf(h).pickFolder).toHaveBeenCalledWith(undefined);
    expect(apiOf(h).saveAll).toHaveBeenCalledWith("C:\\out", [
      { id: "id1", name: "a.pdf" },
      { id: "id3", name: "c.pdf" },
    ]);
    expect(h.store.get("pdf2md.lastFolder")).toBe("C:\\out");
    expect(q(h, "#result").hidden).toBe(false);
    expect(text(h, "#result-summary")).toBe("2 files saved to C:\\out");
    expect(qa(h, "#result-saved-list li").map((l) => l.textContent)).toEqual([
      "OUT\\a.md",
      "OUT\\c.md",
    ]);
    expect(convertBtn(h).disabled).toBe(false);
  });

  it("starts the folder picker in the last used folder", async () => {
    const h = await setup();
    h.store.set("pdf2md.lastFolder", "D:\\notes");
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    convertBtn(h).click();
    await flush();
    expect(apiOf(h).pickFolder).toHaveBeenCalledWith("D:\\notes");
  });

  it("saves nothing when the folder picker is cancelled", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    apiOf(h).pickFolder.mockResolvedValueOnce(null);
    convertBtn(h).click();
    await flush();
    expect(apiOf(h).saveAll).not.toHaveBeenCalled();
    expect(q(h, "#result").hidden).toBe(true);
    expect(convertBtn(h).disabled).toBe(false);
    expect(h.store.has("pdf2md.lastFolder")).toBe(false);
  });

  it("lists failures with their reasons", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf", "C:\\b.pdf");
    h.done(okDone("id1"));
    h.done(okDone("id2"));
    apiOf(h).pickFolder.mockResolvedValueOnce("C:\\ro");
    apiOf(h).saveAll.mockResolvedValueOnce([
      { id: "id1", path: "C:\\ro\\a.md" },
      { id: "id2", error: "Cannot write to C:\\ro: access denied. Choose another folder." },
    ]);
    convertBtn(h).click();
    await flush();
    expect(text(h, "#result-summary")).toBe("1 file saved to C:\\ro. 1 failed.");
    expect(q(h, "#result").dataset.tone).toBe("error");
    const failed = qa(h, "#result-failed li");
    expect(failed).toHaveLength(1);
    expect(failed[0].textContent).toContain("b.pdf");
    expect(failed[0].textContent).toContain("access denied");
  });

  it("reports a rejected save call as a notice and re-enables the button", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    apiOf(h).pickFolder.mockResolvedValueOnce("C:\\out");
    apiOf(h).saveAll.mockRejectedValueOnce("disk exploded");
    convertBtn(h).click();
    await flush();
    expect(text(h, "#notice-text")).toContain("disk exploded");
    expect(text(h, "#notice-text")).toContain("C:\\out");
    expect(convertBtn(h).disabled).toBe(false);
  });

  it("does not run twice on a double click", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    apiOf(h).pickFolder.mockResolvedValue("C:\\out");
    convertBtn(h).click();
    convertBtn(h).click();
    await flush();
    expect(apiOf(h).saveAll).toHaveBeenCalledTimes(1);
  });

  it("the summary can be dismissed", async () => {
    const h = await setup();
    await addViaDialog(h, "C:\\a.pdf");
    h.done(okDone("id1"));
    apiOf(h).pickFolder.mockResolvedValueOnce("C:\\out");
    convertBtn(h).click();
    await flush();
    q(h, "#result-dismiss").click();
    expect(q(h, "#result").hidden).toBe(true);
  });

  it("summarize marks items missing from the backend answer as failed", () => {
    const s = summarize("D", [{ id: "x", name: "x.pdf" }], []);
    expect(s.saved).toEqual([]);
    expect(s.failed[0]).toMatchObject({ id: "x", name: "x.pdf" });
    expect(s.failed[0].error).not.toBe("");
  });
});

describe("accessibility", () => {
  it("has live regions for status that do not announce per-page progress", async () => {
    const h = await setup();
    expect(q(h, "#live").getAttribute("aria-live")).toBe("polite");
    await addViaDialog(h, "C:\\a.pdf");
    expect(text(h, "#live")).toBe("0 of 1 file ready");
    h.progress({ id: "id1", done: 1, total: 9 });
    expect(text(h, "#live")).toBe("0 of 1 file ready");
    h.done(okDone("id1"));
    expect(text(h, "#live")).toBe("1 of 1 file ready");
    expect(q(h, "#notice").getAttribute("role")).toBe("alert");
  });

  it("names the controls", async () => {
    const h = await setup();
    expect(q(h, "#dropzone").getAttribute("aria-label")).toContain("Add PDF files");
    expect(q(h, "#progress-bar").getAttribute("role")).toBe("progressbar");
  });
});
