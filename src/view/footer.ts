import { canConvert, readyFiles, type SaveSummary, type State } from "../state";
import { el, plural, setHidden, setText } from "./dom";
import type { Refs } from "./shell";

export function summaryText(r: SaveSummary): string {
  const saved = r.saved.length;
  const failed = r.failed.length;
  if (saved === 0) {
    return `Nothing was saved: ${plural(failed, "file", "files")} could not be written.`;
  }
  const head = `${plural(saved, "file", "files")} saved to ${r.dir}`;
  return failed > 0 ? `${head}. ${failed} failed.` : head;
}

export function convertHint(s: State): string {
  if (s.saving) return "Saving…";
  if (s.files.length === 0) return "Add a PDF to enable conversion.";
  if (readyFiles(s).length === 0) return "Waiting for a file to finish extracting.";
  return "";
}

function statusText(s: State): string {
  if (s.saving) return "Saving…";
  const total = s.files.length;
  if (total === 0) return "No files yet";
  const ready = readyFiles(s).length;
  return `${ready} of ${plural(total, "file", "files")} ready`;
}

export function renderFooter(r: Refs, s: State): void {
  const enabled = canConvert(s);
  if (r.convert.disabled === enabled) r.convert.disabled = !enabled;
  const hint = convertHint(s);
  if (r.convert.title !== hint) r.convert.title = hint;
  setText(r.convert, s.saving ? "Saving…" : "Convert to Markdown");
  r.convert.setAttribute("aria-busy", s.saving ? "true" : "false");
  setText(r.barStatus, statusText(s));

  const res = s.result;
  setHidden(r.result, res === null);
  if (res === null) {
    if (r.resultSummary.textContent !== "") {
      r.resultSummary.textContent = "";
      r.resultFailed.replaceChildren();
      r.resultSavedList.replaceChildren();
    }
    return;
  }
  r.result.dataset.tone = res.failed.length > 0 ? "error" : "ok";
  setText(r.resultSummary, summaryText(res));

  r.resultFailed.replaceChildren(
    ...res.failed.map((f) => {
      const li = el("li");
      li.append(el("strong", {}, f.name), ` could not be saved: ${f.error}`);
      return li;
    }),
  );
  setHidden(r.resultSaved, res.saved.length === 0);
  setText(r.resultSavedSummary, `Show saved ${res.saved.length === 1 ? "file" : "files"}`);
  r.resultSavedList.replaceChildren(
    ...res.saved.map((f) => {
      const li = el("li");
      li.textContent = f.path;
      return li;
    }),
  );
}
