/** Browser-side helpers that are not Tauri calls: storage and clipboard. */

export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const LAST_FOLDER_KEY = "pdf2md.lastFolder";

/** localStorage can be absent or throw (private mode, blocked data). */
export function defaultStorage(): KeyValueStore | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readLastFolder(store: KeyValueStore | null): string | undefined {
  try {
    return store?.getItem(LAST_FOLDER_KEY) || undefined;
  } catch {
    return undefined;
  }
}

export function writeLastFolder(store: KeyValueStore | null, dir: string): void {
  try {
    store?.setItem(LAST_FOLDER_KEY, dir);
  } catch {
    /* remembering the folder is a convenience only */
  }
}

/** Copy text to the clipboard; rejects when every method fails. */
export async function copyText(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    /* fall through to the legacy path */
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.append(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } finally {
    area.remove();
  }
  if (!ok) throw new Error("Clipboard access was denied");
}
