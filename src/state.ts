/**
 * Pure application state: no DOM, no Tauri. Every transition takes the
 * previous state and returns a new one, so it can be tested in isolation.
 */

export type Status =
  | { kind: "queued" }
  | { kind: "extracting"; done: number; total: number }
  | { kind: "ready"; warnings: string[] }
  | { kind: "password"; wrong: boolean }
  | { kind: "error"; code: string; message: string; hint: string };

export interface FileItem {
  id: string;
  path: string;
  name: string;
  /** Page count, once the engine has reported it. */
  pages: number | null;
  status: Status;
}

export type Preview =
  | { kind: "none" }
  | { kind: "loading"; id: string; seq: number }
  | { kind: "ready"; id: string; text: string }
  | { kind: "error"; id: string; message: string };

/** How the preview shows the Markdown: rendered, or as raw text. */
export type PreviewMode = "markdown" | "raw";

export interface Notice {
  seq: number;
  tone: "info" | "warn" | "error";
  text: string;
}

export interface SavedFile {
  id: string;
  name: string;
  path: string;
}
export interface FailedFile {
  id: string;
  name: string;
  error: string;
}
export interface SaveSummary {
  dir: string;
  saved: SavedFile[];
  failed: FailedFile[];
}

export interface State {
  files: FileItem[];
  selectedId: string | null;
  preview: Preview;
  previewMode: PreviewMode;
  notice: Notice | null;
  saving: boolean;
  result: SaveSummary | null;
  dragging: boolean;
  copied: boolean;
  /** Monotonic counter for notices and preview requests. */
  seq: number;
}

export function initialState(): State {
  return {
    files: [],
    selectedId: null,
    preview: { kind: "none" },
    previewMode: "markdown",
    notice: null,
    saving: false,
    result: null,
    dragging: false,
    copied: false,
    seq: 0,
  };
}

/* ---------- helpers ---------- */

export function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

export function isPdfPath(path: string): boolean {
  return /\.pdf$/i.test(path.trim());
}

/** Key used to detect the same file added twice. */
export function pathKey(path: string): string {
  const unified = path.trim().replace(/\\/g, "/");
  // Windows paths are case-insensitive; POSIX ones are not.
  return /^[a-z]:\//i.test(unified) || unified.startsWith("//")
    ? unified.toLowerCase()
    : unified;
}

export function findFile(state: State, id: string | null): FileItem | undefined {
  return id === null ? undefined : state.files.find((f) => f.id === id);
}

export function selectedFile(state: State): FileItem | undefined {
  return findFile(state, state.selectedId);
}

export function readyFiles(state: State): FileItem[] {
  return state.files.filter((f) => f.status.kind === "ready");
}

/** Convert is available once at least one file is ready (and not mid-save). */
export function canConvert(state: State): boolean {
  return !state.saving && readyFiles(state).length > 0;
}

/** Percentage (0-100) for a file, or null while unknown. */
export function percentOf(file: FileItem): number | null {
  const s = file.status;
  if (s.kind === "ready") return 100;
  if (s.kind === "extracting" && s.total > 0) {
    return Math.min(100, Math.floor((s.done / s.total) * 100));
  }
  return null;
}

export interface Overall {
  /** Files still queued or extracting. */
  active: number;
  /** Files that are ready. */
  ready: number;
  /** Files that count toward the bar (queued, extracting, ready). */
  tracked: number;
  pagesDone: number;
  pagesTotal: number;
  /** 0-100, or null when no page total is known yet. */
  percent: number | null;
}

export function overallProgress(state: State): Overall {
  let active = 0;
  let ready = 0;
  let tracked = 0;
  let pagesDone = 0;
  let pagesTotal = 0;
  for (const f of state.files) {
    const s = f.status;
    if (s.kind === "queued") {
      active++;
      tracked++;
    } else if (s.kind === "extracting") {
      active++;
      tracked++;
      pagesDone += s.done;
      pagesTotal += s.total;
    } else if (s.kind === "ready") {
      ready++;
      tracked++;
      const n = f.pages ?? 0;
      pagesDone += n;
      pagesTotal += n;
    }
  }
  const percent =
    pagesTotal > 0 ? Math.min(100, Math.floor((pagesDone / pagesTotal) * 100)) : null;
  return { active, ready, tracked, pagesDone, pagesTotal, percent };
}

/* ---------- transitions ---------- */

export interface AddResult {
  state: State;
  added: FileItem[];
  rejected: string[];
}

function list(names: string[], max = 3): string {
  if (names.length <= max) return names.join(", ");
  return `${names.slice(0, max).join(", ")} and ${names.length - max} more`;
}

/**
 * Add files by absolute path. Non-PDFs are rejected with a notice naming them;
 * paths already in the list (or repeated within `paths`) are ignored.
 */
export function addPaths(
  state: State,
  paths: string[],
  newId: () => string,
): AddResult {
  const seen = new Set(state.files.map((f) => pathKey(f.path)));
  const added: FileItem[] = [];
  const rejected: string[] = [];
  for (const raw of paths) {
    const path = raw.trim();
    if (!path) continue;
    if (!isPdfPath(path)) {
      rejected.push(fileName(path));
      continue;
    }
    const key = pathKey(path);
    if (seen.has(key)) continue;
    seen.add(key);
    added.push({
      id: newId(),
      path,
      name: fileName(path),
      pages: null,
      status: { kind: "queued" },
    });
  }

  let next: State = { ...state, dragging: false };
  if (added.length > 0) {
    next = { ...next, files: [...state.files, ...added], result: null };
    if (next.selectedId === null) next = selectFile(next, added[0].id);
  }
  if (rejected.length > 0) {
    const many = rejected.length > 1;
    const seq = next.seq + 1;
    next = {
      ...next,
      seq,
      notice: {
        seq,
        tone: "warn",
        text: `${many ? "These files are not PDFs" : "This file is not a PDF"} and ${many ? "were" : "was"} skipped: ${list(rejected)}. Choose files that end in .pdf.`,
      },
    };
  } else if (added.length > 0) {
    next = { ...next, notice: null };
  }
  return { state: next, added, rejected };
}

function previewFor(state: State, file: FileItem | undefined): Preview {
  if (file && file.status.kind === "ready") {
    return { kind: "loading", id: file.id, seq: state.seq + 1 };
  }
  return { kind: "none" };
}

export function selectFile(state: State, id: string): State {
  const file = findFile(state, id);
  if (!file) return state;
  if (state.selectedId === id && state.preview.kind !== "none") return state;
  const preview = previewFor(state, file);
  return {
    ...state,
    selectedId: id,
    preview,
    copied: false,
    seq: preview.kind === "loading" ? preview.seq : state.seq,
  };
}

export function removeFile(state: State, id: string): State {
  const index = state.files.findIndex((f) => f.id === id);
  if (index < 0) return state;
  const files = state.files.filter((f) => f.id !== id);
  let next: State = { ...state, files };
  if (state.selectedId === id) {
    const fallback = files[index] ?? files[index - 1];
    next = { ...next, selectedId: null, preview: { kind: "none" }, copied: false };
    if (fallback) next = selectFile(next, fallback.id);
  }
  return next;
}

export function clearAll(state: State): State {
  return {
    ...state,
    files: [],
    selectedId: null,
    preview: { kind: "none" },
    result: null,
    copied: false,
  };
}

/** Engine progress for one file. Ignored for unknown or finished files. */
export function progressed(
  state: State,
  id: string,
  done: number,
  total: number,
): State {
  const file = findFile(state, id);
  if (!file) return state;
  if (file.status.kind !== "queued" && file.status.kind !== "extracting") return state;
  const prev = file.status.kind === "extracting" ? file.status : null;
  // Events can arrive slightly out of order; never move backwards.
  const nextDone = prev && prev.total === total ? Math.max(prev.done, done) : done;
  return updateFile(state, id, {
    pages: total > 0 ? total : file.pages,
    status: { kind: "extracting", done: nextDone, total },
  });
}

export interface DonePayload {
  id: string;
  ok: boolean;
  pages?: number;
  warnings?: string[];
  error?: { code: string; message: string; hint: string };
}

/** Terminal result for a file's extraction. */
export function finished(state: State, e: DonePayload): State {
  const file = findFile(state, e.id);
  if (!file) return state;
  if (e.ok) {
    let next = updateFile(state, e.id, {
      pages: e.pages ?? file.pages,
      status: { kind: "ready", warnings: e.warnings ?? [] },
    });
    if (next.selectedId === e.id) {
      const preview = previewFor(next, findFile(next, e.id));
      next = {
        ...next,
        preview,
        seq: preview.kind === "loading" ? preview.seq : next.seq,
      };
    }
    return next;
  }
  const err = e.error ?? {
    code: "internal",
    message: "The conversion failed for an unknown reason.",
    hint: "Remove the file and add it again.",
  };
  let status: Status;
  if (err.code === "password_required") status = { kind: "password", wrong: false };
  else if (err.code === "wrong_password") status = { kind: "password", wrong: true };
  else status = { kind: "error", code: err.code, message: err.message, hint: err.hint };
  let next = updateFile(state, e.id, { status });
  if (next.selectedId === e.id) next = { ...next, preview: { kind: "none" } };
  return next;
}

/** The user submitted a password: the file goes back to extracting. */
export function retryWithPassword(state: State, id: string): State {
  const file = findFile(state, id);
  if (!file || file.status.kind !== "password") return state;
  return updateFile(state, id, {
    status: { kind: "extracting", done: 0, total: file.pages ?? 0 },
  });
}

/** The backend refused to start extraction (IPC failure). */
export function extractFailed(state: State, id: string, message: string): State {
  return finished(state, {
    id,
    ok: false,
    error: {
      code: "internal",
      message,
      hint: "Remove the file and add it again. If this keeps happening, restart PDF2MD.",
    },
  });
}

export function previewLoaded(state: State, id: string, seq: number, text: string): State {
  const p = state.preview;
  if (p.kind !== "loading" || p.id !== id || p.seq !== seq) return state;
  return { ...state, preview: { kind: "ready", id, text } };
}

export function previewFailed(state: State, id: string, seq: number, message: string): State {
  const p = state.preview;
  if (p.kind !== "loading" || p.id !== id || p.seq !== seq) return state;
  return { ...state, preview: { kind: "error", id, message } };
}

export function previewRetry(state: State): State {
  const file = selectedFile(state);
  if (!file) return state;
  const preview = previewFor(state, file);
  return {
    ...state,
    preview,
    seq: preview.kind === "loading" ? preview.seq : state.seq,
  };
}

export function togglePreviewMode(state: State): State {
  return { ...state, previewMode: state.previewMode === "markdown" ? "raw" : "markdown" };
}

export function setDragging(state: State, dragging: boolean): State {
  return state.dragging === dragging ? state : { ...state, dragging };
}

export function setCopied(state: State, copied: boolean): State {
  return state.copied === copied ? state : { ...state, copied };
}

export function setNotice(state: State, tone: Notice["tone"], text: string): State {
  const seq = state.seq + 1;
  return { ...state, seq, notice: { seq, tone, text } };
}

export function dismissNotice(state: State): State {
  return state.notice ? { ...state, notice: null } : state;
}

export function savingStarted(state: State): State {
  return { ...state, saving: true, result: null };
}

export function savingFinished(state: State, result: SaveSummary | null): State {
  return { ...state, saving: false, result };
}

export function dismissResult(state: State): State {
  return state.result ? { ...state, result: null } : state;
}

function updateFile(state: State, id: string, patch: Partial<FileItem>): State {
  return {
    ...state,
    files: state.files.map((f) => (f.id === id ? { ...f, ...patch } : f)),
  };
}

/* ---------- store ---------- */

export type Listener = (state: State) => void;

export interface Store {
  get(): State;
  /** Apply a pure transition; listeners run only when the state changed. */
  update(fn: (s: State) => State): State;
  subscribe(fn: Listener): () => void;
}

export function createStore(initial: State = initialState()): Store {
  let state = initial;
  const listeners = new Set<Listener>();
  return {
    get: () => state,
    update(fn) {
      const next = fn(state);
      if (next !== state) {
        state = next;
        for (const l of [...listeners]) l(state);
      }
      return state;
    },
    subscribe(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
  };
}
