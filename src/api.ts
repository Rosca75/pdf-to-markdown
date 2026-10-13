/**
 * The ONLY module that imports @tauri-apps/*. Everything else talks to the
 * backend through the `Api` interface, so tests can pass a mock.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import type { DonePayload } from "./state";

export interface ProgressPayload {
  id: string;
  done: number;
  total: number;
}

export type { DonePayload };

export interface SaveItem {
  id: string;
  name: string;
}

export interface SaveOutcome {
  id: string;
  path?: string;
  error?: string;
}

export type DragEvent =
  | { type: "enter"; paths: string[] }
  | { type: "over" }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };

export type Unlisten = () => void;

export interface Api {
  /** Native file dialog; resolves to the chosen PDF paths (empty when cancelled). */
  pickPdfs(): Promise<string[]>;
  /** Native folder dialog; resolves to null when cancelled. */
  pickFolder(defaultPath?: string): Promise<string | null>;
  extract(id: string, path: string, password?: string): Promise<void>;
  getMarkdown(id: string): Promise<string>;
  remove(id: string): Promise<void>;
  saveAll(dir: string, items: SaveItem[]): Promise<SaveOutcome[]>;
  onProgress(cb: (p: ProgressPayload) => void): Promise<Unlisten>;
  onDone(cb: (d: DonePayload) => void): Promise<Unlisten>;
  onDragDrop(cb: (e: DragEvent) => void): Promise<Unlisten>;
}

/** Tauri rejects with the command's error string (or an Error); flatten it. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export const tauriApi: Api = {
  async pickPdfs() {
    const picked = await open({
      multiple: true,
      directory: false,
      title: "Choose PDF files",
      filters: [{ name: "PDF documents", extensions: ["pdf"] }],
    });
    if (!picked) return [];
    return Array.isArray(picked) ? picked : [picked];
  },

  async pickFolder(defaultPath) {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Choose where to save the Markdown files",
      defaultPath: defaultPath || undefined,
    });
    return typeof picked === "string" ? picked : null;
  },

  async extract(id, path, password) {
    await invoke("extract", { id, path, password: password ?? null });
  },

  getMarkdown: (id) => invoke<string>("get_markdown", { id }),

  async remove(id) {
    await invoke("remove", { id });
  },

  saveAll: (dir, items) => invoke<SaveOutcome[]>("save_all", { dir, items }),

  onProgress: (cb) => listen<ProgressPayload>("extract://progress", (e) => cb(e.payload)),

  onDone: (cb) => listen<DonePayload>("extract://done", (e) => cb(e.payload)),

  onDragDrop: (cb) =>
    getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "drop") cb({ type: p.type, paths: p.paths });
      else if (p.type === "over") cb({ type: "over" });
      else cb({ type: "leave" });
    }),
};
