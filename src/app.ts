/**
 * Wiring: connects the pure store, the views and the backend API.
 * `mount` takes the API as a parameter so tests can pass a mock.
 */
import { errorText, type Api, type SaveOutcome } from "./api";
import {
  addPaths,
  canConvert,
  clearAll,
  createStore,
  dismissNotice,
  dismissResult,
  extractFailed,
  finished,
  previewFailed,
  previewLoaded,
  previewRetry,
  progressed,
  readyFiles,
  removeFile,
  retryWithPassword,
  savingFinished,
  savingStarted,
  selectFile,
  setCopied,
  setDragging,
  setNotice,
  togglePreviewMode,
  type FileItem,
  type SaveSummary,
  type Store,
} from "./state";
import {
  copyText,
  defaultStorage,
  readLastFolder,
  writeLastFolder,
  type KeyValueStore,
} from "./platform";
import { createView, type View } from "./view";

export interface MountOptions {
  api: Api;
  storage?: KeyValueStore | null;
  newId?: () => string;
  copy?: (text: string) => Promise<void>;
}

export interface App {
  store: Store;
  view: View;
  dispose(): void;
}

const COPIED_MS = 1600;

export function summarize(
  dir: string,
  snapshot: { id: string; name: string }[],
  outcomes: SaveOutcome[],
): SaveSummary {
  const byId = new Map(outcomes.map((o) => [o.id, o]));
  const summary: SaveSummary = { dir, saved: [], failed: [] };
  for (const item of snapshot) {
    const o = byId.get(item.id);
    if (o?.path) summary.saved.push({ id: item.id, name: item.name, path: o.path });
    else {
      summary.failed.push({
        id: item.id,
        name: item.name,
        error: o?.error ?? "The file was not saved. Try again.",
      });
    }
  }
  return summary;
}

export async function mount(root: HTMLElement, opts: MountOptions): Promise<App> {
  const { api } = opts;
  const storage = opts.storage === undefined ? defaultStorage() : opts.storage;
  const copy = opts.copy ?? copyText;
  let counter = 0;
  const newId =
    opts.newId ?? (() => `f${++counter}-${Math.random().toString(36).slice(2, 8)}`);

  const store = createStore();
  const view = createView(root);
  const { refs } = view;
  const disposers: (() => void)[] = [];
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  let lastPreviewSeq = 0;
  let picking = false;

  const unsubscribe = store.subscribe((s) => {
    view.render(s);
    requestPreview();
  });
  disposers.push(unsubscribe);

  /* ----- preview loading ----- */

  function requestPreview(): void {
    const p = store.get().preview;
    if (p.kind !== "loading" || p.seq <= lastPreviewSeq) return;
    lastPreviewSeq = p.seq;
    api.getMarkdown(p.id).then(
      (text) => store.update((s) => previewLoaded(s, p.id, p.seq, text)),
      (e) => store.update((s) => previewFailed(s, p.id, p.seq, errorText(e))),
    );
  }

  /* ----- adding files ----- */

  function startExtract(file: FileItem, password?: string): void {
    api.extract(file.id, file.path, password).catch((e) => {
      store.update((s) => extractFailed(s, file.id, `Extraction could not start (${errorText(e)}).`));
    });
  }

  function add(paths: string[]): void {
    let added: FileItem[] = [];
    store.update((s) => {
      const r = addPaths(s, paths, newId);
      added = r.added;
      return r.state;
    });
    for (const f of added) startExtract(f);
  }

  async function browse(): Promise<void> {
    if (picking) return;
    picking = true;
    try {
      const paths = await api.pickPdfs();
      if (paths.length > 0) add(paths);
    } catch (e) {
      store.update((s) =>
        setNotice(s, "error", `The file dialog could not be opened (${errorText(e)}). Try dragging the files in instead.`),
      );
    } finally {
      picking = false;
    }
  }

  refs.dropzone.addEventListener("click", () => void browse());
  refs.dropzone.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " " || e.key === "Spacebar") {
      e.preventDefault();
      void browse();
    }
  });

  /* ----- file list (delegated) ----- */

  function rowId(target: EventTarget | null): string | null {
    const li = (target as HTMLElement | null)?.closest?.("li[data-id]");
    return li ? li.getAttribute("data-id") : null;
  }

  refs.files.addEventListener("click", (e) => {
    const target = e.target as HTMLElement;
    const actionEl = target.closest("[data-action]");
    const id = rowId(target);
    if (!actionEl || !id) return;
    const action = actionEl.getAttribute("data-action");
    if (action === "select") {
      store.update((s) => selectFile(s, id));
    } else if (action === "remove") {
      void api.remove(id).catch(() => undefined);
      store.update((s) => removeFile(s, id));
    }
  });

  refs.files.addEventListener("submit", (e) => {
    e.preventDefault();
    const form = e.target as HTMLFormElement;
    const id = rowId(form);
    if (!id) return;
    const input = form.querySelector<HTMLInputElement>("input");
    const password = input?.value ?? "";
    if (!input || password === "") {
      input?.focus();
      return;
    }
    const file = store.get().files.find((f) => f.id === id);
    if (!file) return;
    store.update((s) => retryWithPassword(s, id));
    startExtract(file, password);
  });

  refs.clearAll.addEventListener("click", () => {
    for (const f of store.get().files) void api.remove(f.id).catch(() => undefined);
    store.update(clearAll);
  });

  /* ----- notices, result, preview actions ----- */

  refs.noticeDismiss.addEventListener("click", () => store.update(dismissNotice));
  refs.resultDismiss.addEventListener("click", () => store.update(dismissResult));
  refs.pvRetry.addEventListener("click", () => store.update(previewRetry));
  refs.pvMode.addEventListener("click", () => store.update(togglePreviewMode));

  refs.copy.addEventListener("click", () => {
    const p = store.get().preview;
    if (p.kind !== "ready") return;
    copy(p.text).then(
      () => {
        store.update((s) => setCopied(s, true));
        clearTimeout(copiedTimer);
        copiedTimer = setTimeout(() => store.update((s) => setCopied(s, false)), COPIED_MS);
      },
      () =>
        store.update((s) =>
          setNotice(s, "error", "Copy failed. Click into the preview, press Ctrl+A then Ctrl+C (Cmd+A, Cmd+C on a Mac)."),
        ),
    );
  });

  /* ----- convert ----- */

  refs.convert.addEventListener("click", async () => {
    if (!canConvert(store.get())) return;
    const snapshot = readyFiles(store.get()).map((f) => ({ id: f.id, name: f.name }));
    store.update(savingStarted);
    let dir: string | null = null;
    try {
      dir = await api.pickFolder(readLastFolder(storage));
      if (!dir) {
        store.update((s) => savingFinished(s, null));
        return;
      }
      writeLastFolder(storage, dir);
      const outcomes = await api.saveAll(dir, snapshot);
      const summary = summarize(dir, snapshot, outcomes);
      store.update((s) => savingFinished(s, summary));
    } catch (e) {
      const where = dir ? ` to ${dir}` : "";
      store.update((s) =>
        setNotice(
          savingFinished(s, null),
          "error",
          `Could not save${where}: ${errorText(e)}. Choose another folder and try again.`,
        ),
      );
    }
  });

  /* ----- window-level drag and drop ----- */

  const swallow = (e: Event) => e.preventDefault();
  window.addEventListener("dragover", swallow);
  window.addEventListener("drop", swallow);
  disposers.push(() => {
    window.removeEventListener("dragover", swallow);
    window.removeEventListener("drop", swallow);
  });

  /* ----- backend events ----- */

  const listeners = await Promise.all([
    api.onProgress((p) => store.update((s) => progressed(s, p.id, p.done, p.total))),
    api.onDone((d) => store.update((s) => finished(s, d))),
    api.onDragDrop((e) => {
      if (e.type === "enter") store.update((s) => setDragging(s, true));
      else if (e.type === "leave") store.update((s) => setDragging(s, false));
      else if (e.type === "drop") add(e.paths);
    }),
  ]);
  disposers.push(...listeners);

  view.render(store.get());

  return {
    store,
    view,
    dispose() {
      clearTimeout(copiedTimer);
      for (const d of disposers) d();
    },
  };
}
