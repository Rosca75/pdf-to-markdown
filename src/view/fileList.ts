import { percentOf, type FileItem, type State, type Status } from "../state";
import { el, plural, setAttr, setText } from "./dom";

/**
 * Keyed, in-place file list. Rows are created once per file and patched on
 * every state change, so a focused password field or button survives the
 * ~20 progress updates per second.
 */

interface Row {
  li: HTMLLIElement;
  main: HTMLButtonElement;
  chip: HTMLElement;
  meta: HTMLElement;
  extra: HTMLElement;
  /** Status kind the `extra` block was built for. */
  extraKind: Status["kind"] | null;
  /** Elements inside `extra` that get patched while the kind stays the same. */
  fill?: HTMLElement;
  bar?: HTMLElement;
  pwMessage?: HTMLElement;
  pwWrong?: boolean;
}

const rows = new WeakMap<HTMLElement, Map<string, Row>>();

export const PASSWORD_HELP = "This PDF is password protected. Enter the password to unlock it.";
export const PASSWORD_WRONG = "That password is incorrect. Check it (it is case-sensitive) and try again.";

export function chipText(file: FileItem): string {
  const s = file.status;
  switch (s.kind) {
    case "queued":
      return "Queued";
    case "extracting": {
      const p = percentOf(file);
      return p === null ? "Extracting" : `Extracting ${p}%`;
    }
    case "ready":
      return "Ready";
    case "password":
      return "Password required";
    case "error":
      return "Failed";
  }
}

function metaText(file: FileItem): string {
  const parts: string[] = [];
  if (file.pages !== null && file.pages > 0) parts.push(plural(file.pages, "page", "pages"));
  if (file.status.kind === "ready" && file.status.warnings.length > 0) {
    parts.push(plural(file.status.warnings.length, "warning", "warnings"));
  }
  return parts.join(" · ");
}

function buildRow(file: FileItem): Row {
  const main = el("button", { type: "button", class: "file-main", "data-action": "select" });
  const dot = el("span", { class: "dot", "aria-hidden": "true" });
  const name = el("span", { class: "file-name" });
  name.textContent = file.name;
  const chip = el("span", { class: "chip" });
  const meta = el("span", { class: "file-meta" });
  main.append(dot, name, chip, meta);
  main.title = file.path;

  const remove = el(
    "button",
    { type: "button", class: "icon-btn file-remove", "data-action": "remove" },
    "×",
  );
  remove.setAttribute("aria-label", `Remove ${file.name}`);
  remove.title = "Remove from list";

  const extra = el("div", { class: "file-extra" });
  const li = el("li", { class: "file", "data-id": file.id }, main, remove, extra);
  return { li, main, chip, meta, extra, extraKind: null };
}

function buildExtra(row: Row, file: FileItem): void {
  const s = file.status;
  row.extra.replaceChildren();
  row.fill = row.bar = row.pwMessage = undefined;
  row.pwWrong = undefined;
  row.extraKind = s.kind;
  row.extra.hidden = s.kind === "queued" || s.kind === "ready";

  if (s.kind === "extracting") {
    const fill = el("div", { class: "bar-fill" });
    const bar = el(
      "div",
      {
        class: "bar-track bar-small",
        role: "progressbar",
        "aria-label": `Extraction progress for ${file.name}`,
        "aria-valuemin": "0",
        "aria-valuemax": "100",
      },
      fill,
    );
    row.extra.append(bar);
    row.fill = fill;
    row.bar = bar;
  } else if (s.kind === "password") {
    const message = el("p", { class: "file-note", role: "alert" });
    const input = el("input", {
      type: "password",
      class: "pw-input",
      autocomplete: "off",
      spellcheck: "false",
      placeholder: "Password",
      "aria-label": `Password for ${file.name}`,
    });
    const submit = el("button", { type: "submit", class: "btn btn-small" }, "Unlock");
    const form = el("form", { class: "pw-form", "data-action": "unlock" }, input, submit);
    row.extra.append(message, form);
    row.pwMessage = message;
  } else if (s.kind === "error") {
    row.extra.append(
      el("p", { class: "file-note file-error" }, s.message),
      el("p", { class: "file-hint" }, s.hint),
    );
  }
}

function patchRow(row: Row, file: FileItem, selected: boolean): void {
  const s = file.status;
  setAttr(row.li, "data-status", s.kind);
  row.li.classList.toggle("is-selected", selected);
  setAttr(row.main, "aria-current", selected ? "true" : null);
  setText(row.chip, chipText(file));
  setText(row.meta, metaText(file));

  if (row.extraKind !== s.kind) {
    const hadFocus = row.extra.contains(document.activeElement);
    buildExtra(row, file);
    // A wrong password re-enters this state; put the user back in the field.
    if (s.kind === "password" && (hadFocus || s.wrong)) {
      row.extra.querySelector<HTMLInputElement>("input")?.focus();
    }
  }

  if (s.kind === "extracting" && row.fill && row.bar) {
    const p = percentOf(file);
    row.bar.classList.toggle("is-indeterminate", p === null);
    if (p === null) {
      setAttr(row.bar, "aria-valuenow", null);
      row.fill.style.width = "";
    } else {
      setAttr(row.bar, "aria-valuenow", String(p));
      const w = `${p}%`;
      if (row.fill.style.width !== w) row.fill.style.width = w;
    }
  } else if (s.kind === "password" && row.pwMessage) {
    if (row.pwWrong !== s.wrong) {
      row.pwWrong = s.wrong;
      setText(row.pwMessage, s.wrong ? PASSWORD_WRONG : PASSWORD_HELP);
    }
  }
}

export function renderFileList(ul: HTMLUListElement, state: State): void {
  let map = rows.get(ul);
  if (!map) {
    map = new Map();
    rows.set(ul, map);
  }
  const live = new Set<string>();
  state.files.forEach((file, index) => {
    live.add(file.id);
    let row = map.get(file.id);
    if (!row) {
      row = buildRow(file);
      map.set(file.id, row);
    }
    patchRow(row, file, file.id === state.selectedId);
    if (ul.children[index] !== row.li) ul.insertBefore(row.li, ul.children[index] ?? null);
  });
  for (const [id, row] of map) {
    if (!live.has(id)) {
      row.li.remove();
      map.delete(id);
    }
  }
}
