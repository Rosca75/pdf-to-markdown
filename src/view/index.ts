import type { State } from "../state";
import { readyFiles } from "../state";
import { plural, setText } from "./dom";
import { renderFileList } from "./fileList";
import { renderFooter } from "./footer";
import { renderInputPanel } from "./inputPanel";
import { renderPreview } from "./preview";
import { buildShell, type Refs } from "./shell";

export type { Refs } from "./shell";

export interface View {
  refs: Refs;
  render(state: State): void;
}

export function createView(root: HTMLElement): View {
  const refs = buildShell(root);
  return {
    refs,
    render(state) {
      renderInputPanel(refs, state);
      renderFileList(refs.files, state);
      renderPreview(refs, state);
      renderFooter(refs, state);
      // Screen readers hear file-level changes only, never per-page progress.
      const total = state.files.length;
      setText(
        refs.live,
        total === 0 ? "" : `${readyFiles(state).length} of ${plural(total, "file", "files")} ready`,
      );
    },
  };
}
