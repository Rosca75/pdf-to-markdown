/**
 * Renders the Markdown subset emitted by the engine (crates/pdf2md-engine/src/render.rs)
 * to DOM nodes. Hand-written on purpose: no dependency, no innerHTML, and links are
 * never clickable (no href), so untrusted PDF content cannot navigate or run script.
 */

const NL = /\r?\n/;
const DELIM_CELL = /^:?-+:?$/;

/** Render the engine's Markdown subset to DOM. Never uses innerHTML. */
export function renderMarkdown(md: string): DocumentFragment {
  const frag = document.createDocumentFragment();
  if (!md) return frag;
  const lines = md.split(NL);
  if (lines[lines.length - 1] === "") lines.pop(); // the final newline ends the last line

  let para: string[] = [];
  let lists: ListFrame[] = [];

  const flushPara = (): void => {
    if (para.length === 0) return;
    const p = document.createElement("p");
    renderInline(para.join(" "), p);
    frag.appendChild(p);
    para = [];
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === "") {
      flushPara();
      lists = [];
      continue;
    }

    const fence = fenceOpen(line);
    if (fence > 0) {
      flushPara();
      lists = [];
      const body: string[] = [];
      i++;
      // An unclosed fence runs to the end of the document.
      while (i < lines.length && !fenceClose(lines[i], fence)) body.push(lines[i++]);
      const pre = document.createElement("pre");
      const code = document.createElement("code");
      code.textContent = body.join("\n");
      pre.appendChild(code);
      frag.appendChild(pre);
      continue;
    }

    const heading = parseHeading(line);
    if (heading) {
      flushPara();
      lists = [];
      const h = document.createElement(`h${heading.level}`);
      renderInline(heading.text, h);
      frag.appendChild(h);
      continue;
    }

    if (i + 1 < lines.length && isTableStart(line, lines[i + 1])) {
      flushPara();
      lists = [];
      const rows = [splitRow(line)];
      i += 2; // header and delimiter row
      while (i < lines.length && lines[i].trimStart().startsWith("|")) {
        rows.push(splitRow(lines[i++]));
      }
      i--;
      frag.appendChild(buildTable(rows));
      continue;
    }

    const item = parseItem(line);
    if (item && (lists.length > 0 || item.indent <= 3)) {
      flushPara();
      addItem(frag, lists, item);
      continue;
    }

    if (para.length === 0) lists = [];
    para.push(line.trim());
  }
  flushPara();
  return frag;
}

// ---------------------------------------------------------------- blocks

/** Length of the opening backtick fence on this line, or 0. */
function fenceOpen(line: string): number {
  let i = 0;
  while (i < 3 && line.charCodeAt(i) === 32) i++;
  let n = 0;
  while (line.charCodeAt(i + n) === 96) n++;
  if (n < 3) return 0;
  return line.indexOf("`", i + n) >= 0 ? 0 : n; // info string may not hold backticks
}

function fenceClose(line: string, open: number): boolean {
  const t = line.trim();
  if (t.length < open) return false;
  for (let i = 0; i < t.length; i++) if (t.charCodeAt(i) !== 96) return false;
  return true;
}

function parseHeading(line: string): { level: number; text: string } | null {
  let i = 0;
  while (i < 3 && line.charCodeAt(i) === 32) i++;
  let level = 0;
  while (line.charCodeAt(i + level) === 35) level++;
  if (level < 1 || level > 6) return null;
  const after = line.charCodeAt(i + level);
  if (!Number.isNaN(after) && after !== 32 && after !== 9) return null;
  return { level, text: line.slice(i + level).trim() };
}

interface Item {
  indent: number;
  /** "-", "+", "*" for bullets; "." or ")" for ordered. */
  key: string;
  start: number;
  text: string;
}

function parseItem(line: string): Item | null {
  let i = 0;
  let indent = 0;
  for (; i < line.length; i++) {
    const c = line.charCodeAt(i);
    if (c === 32) indent++;
    else if (c === 9) indent += 4;
    else break;
  }
  const c = line.charCodeAt(i);
  let key: string;
  let start = 1;
  let j = i + 1;
  if (c === 45 || c === 43 || c === 42) {
    key = line[i];
  } else if (c >= 48 && c <= 57) {
    while (j < line.length && j - i < 10 && isDigit(line.charCodeAt(j))) j++;
    if (j - i > 9) return null;
    const m = line.charCodeAt(j);
    if (m !== 46 && m !== 41) return null;
    key = line[j];
    start = Number(line.slice(i, j));
    j++;
  } else {
    return null;
  }
  const next = line.charCodeAt(j);
  if (!Number.isNaN(next) && next !== 32 && next !== 9) return null;
  return { indent, key, start, text: line.slice(j).trim() };
}

interface ListFrame {
  indent: number;
  key: string;
  el: HTMLElement;
  li: HTMLElement;
}

/**
 * Place an item into the open list tree. An item indented 2+ columns deeper than the
 * innermost list nests inside its last item; a different marker at the same level
 * starts a new sibling list, as CommonMark does.
 */
function addItem(root: Node, lists: ListFrame[], item: Item): void {
  while (lists.length > 0 && lists[lists.length - 1].indent > item.indent) lists.pop();
  let top: ListFrame | undefined = lists[lists.length - 1];
  let container: Node = root;
  if (top && item.indent >= top.indent + 2) {
    container = top.li;
    top = undefined;
  } else if (top && top.key !== item.key) {
    lists.pop();
    top = undefined;
    container = lists.length > 0 ? lists[lists.length - 1].li : root;
  } else if (!top) {
    container = root;
  }

  if (!top) {
    const ordered = item.key === "." || item.key === ")";
    const el = document.createElement(ordered ? "ol" : "ul");
    if (ordered && item.start !== 1) el.setAttribute("start", String(item.start));
    container.appendChild(el);
    top = { indent: item.indent, key: item.key, el, li: el };
    lists.push(top);
  }
  const li = document.createElement("li");
  top.el.appendChild(li);
  top.li = li;
  renderInline(item.text, li);
}

function isDigit(c: number): boolean {
  return c >= 48 && c <= 57;
}

/** Split a table row on unescaped pipes; `\|` becomes `|` in the cell text. */
function splitRow(line: string): string[] {
  const t = line.trim();
  const cells: string[] = [];
  let cur = t.charCodeAt(0) === 124 ? 1 : 0;
  for (let j = cur; j < t.length; j++) {
    const c = t.charCodeAt(j);
    if (c === 92) {
      j++;
    } else if (c === 124) {
      cells.push(unescapePipes(t.slice(cur, j)).trim());
      cur = j + 1;
    }
  }
  if (cur < t.length) cells.push(unescapePipes(t.slice(cur)).trim());
  return cells;
}

function unescapePipes(s: string): string {
  if (s.indexOf("\\") < 0) return s;
  let out = "";
  let lit = 0;
  for (let i = 0; i < s.length; i++) {
    if (s.charCodeAt(i) !== 92) continue;
    if (s.charCodeAt(i + 1) === 124) {
      out += s.slice(lit, i);
      lit = i + 1;
    }
    i++; // an escaped backslash must not escape what follows it
  }
  return out + s.slice(lit);
}

function isTableStart(line: string, next: string): boolean {
  if (!line.trimStart().startsWith("|") || !next.trimStart().startsWith("|")) return false;
  const cells = splitRow(next);
  return cells.length > 0 && cells.every((c) => DELIM_CELL.test(c));
}

function buildTable(rows: string[][]): HTMLElement {
  let cols = 0;
  for (const r of rows) cols = Math.max(cols, r.length);
  const table = document.createElement("table");
  const thead = document.createElement("thead");
  const tbody = document.createElement("tbody");
  rows.forEach((cells, r) => {
    const tr = document.createElement("tr");
    for (let c = 0; c < cols; c++) {
      const cell = document.createElement(r === 0 ? "th" : "td");
      if (c < cells.length) renderInline(cells[c], cell);
      tr.appendChild(cell);
    }
    (r === 0 ? thead : tbody).appendChild(tr);
  });
  table.appendChild(thead);
  if (rows.length > 1) table.appendChild(tbody);
  return table;
}

// --------------------------------------------------------------- inlines

interface Cache {
  from: number;
  res: number;
}

/**
 * Per-range scanner state. The memos record where a closer search already failed so a
 * line full of unmatched openers is scanned once, not once per opener.
 */
interface Scan {
  s: string;
  hi: number;
  starFail: number[];
  codeFail: Map<number, number> | null;
  bracket: Cache;
  paren: Cache;
}

function renderInline(text: string, parent: Node): void {
  parseRange(newScan(text, text.length), 0, parent);
}

function newScan(s: string, hi: number): Scan {
  return {
    s,
    hi,
    starFail: [Infinity, Infinity, Infinity, Infinity],
    codeFail: null,
    bracket: { from: -1, res: -1 },
    paren: { from: -1, res: -1 },
  };
}

function isSpace(c: number): boolean {
  return c <= 32 || c === 0xa0;
}

function isPunct(c: number): boolean {
  return (c >= 33 && c <= 47) || (c >= 58 && c <= 64) || (c >= 91 && c <= 96) || (c >= 123 && c <= 126);
}

/** Parse sc.s[lo, sc.hi) into `parent`. Anything unrecognised stays literal text. */
function parseRange(sc: Scan, lo: number, parent: Node): void {
  const { s, hi } = sc;
  let out = ""; // literal text pending for the next text node
  let lit = lo; // start of the not-yet-copied literal slice
  let i = lo;

  const flush = (upto: number): void => {
    out += s.slice(lit, upto);
    if (out) parent.appendChild(document.createTextNode(out));
    out = "";
  };

  while (i < hi) {
    const c = s.charCodeAt(i);
    if (c === 92) {
      if (i + 1 < hi && isPunct(s.charCodeAt(i + 1))) {
        out += s.slice(lit, i); // drop the backslash, keep the escaped char literal
        lit = i + 1;
        i += 2;
      } else {
        i++;
      }
    } else if (c === 96) {
      const n = runLength(s, i, hi, 96);
      const close = findCodeClose(sc, i + n, n);
      if (close < 0) {
        i += n;
        continue;
      }
      flush(i);
      const code = document.createElement("code");
      code.textContent = codeContent(s.slice(i + n, close));
      parent.appendChild(code);
      i = lit = close + n;
    } else if (c === 42) {
      const k = runLength(s, i, hi, 42);
      const close = k <= 3 && i + k < hi && !isSpace(s.charCodeAt(i + k)) ? findStarClose(sc, i + k, k) : -1;
      if (close < 0) {
        i += k;
        continue;
      }
      flush(i);
      let inner: Node = parent;
      if (k >= 2) inner = parent.appendChild(document.createElement("strong"));
      if (k !== 2) inner = inner.appendChild(document.createElement("em"));
      parseRange(newScan(s, close), i + k, inner);
      i = lit = close + k;
    } else if (c === 91) {
      const link = tryLink(sc, i);
      if (!link) {
        i++;
        continue;
      }
      flush(i);
      const a = document.createElement("a");
      a.className = "md-link"; // deliberately no href: links are not clickable
      if (link.url) a.setAttribute("title", decodeUrl(link.url));
      parseRange(newScan(s, link.textEnd), i + 1, a);
      parent.appendChild(a);
      i = lit = link.end;
    } else {
      i++;
    }
  }
  flush(hi);
}

function runLength(s: string, i: number, hi: number, ch: number): number {
  let n = 0;
  while (i + n < hi && s.charCodeAt(i + n) === ch) n++;
  return n;
}

/** Strip one leading and trailing space when both exist and the span is not all spaces. */
function codeContent(raw: string): string {
  if (raw.length >= 2 && raw[0] === " " && raw[raw.length - 1] === " " && raw.trim() !== "") {
    return raw.slice(1, -1);
  }
  return raw;
}

/** Start of the next backtick run of exactly `n` at or after `p`, or -1. */
function findCodeClose(sc: Scan, p: number, n: number): number {
  const failed = sc.codeFail?.get(n);
  if (failed !== undefined && p >= failed) return -1;
  const { s, hi } = sc;
  let j = p;
  while (j < hi) {
    j = s.indexOf("`", j);
    if (j < 0 || j >= hi) break;
    const m = runLength(s, j, hi, 96);
    if (m === n) return j;
    j += m;
  }
  (sc.codeFail ??= new Map()).set(n, p);
  return -1;
}

/** Start of the next star run of at least `k` that can close emphasis, or -1. */
function findStarClose(sc: Scan, p: number, k: number): number {
  if (p >= sc.starFail[k]) return -1;
  const { s, hi } = sc;
  let j = p;
  while (j < hi) {
    const c = s.charCodeAt(j);
    if (c === 92) {
      j += 2;
    } else if (c === 96) {
      const n = runLength(s, j, hi, 96);
      const close = findCodeClose(sc, j + n, n);
      j = close < 0 ? j + n : close + n;
    } else if (c === 42) {
      const m = runLength(s, j, hi, 42);
      // A longer run closes with its first k stars and opens the next run with
      // the rest: the engine glues differently styled runs (`**a***b*`).
      if (m >= k && j > p && !isSpace(s.charCodeAt(j - 1))) return j;
      j += m;
    } else {
      j++;
    }
  }
  sc.starFail[k] = p;
  return -1;
}

/** Reuse a previous search result when it is still the first hit for position p. */
function cached(cache: Cache, p: number): number | undefined {
  if (cache.from >= 0 && p >= cache.from && (cache.res < 0 || p <= cache.res)) return cache.res;
  return undefined;
}

function findBracketClose(sc: Scan, p: number): number {
  const hit = cached(sc.bracket, p);
  if (hit !== undefined) return hit;
  const { s, hi } = sc;
  let res = -1;
  for (let j = p; j < hi; ) {
    const c = s.charCodeAt(j);
    if (c === 92) {
      j += 2;
    } else if (c === 96) {
      const n = runLength(s, j, hi, 96);
      const close = findCodeClose(sc, j + n, n);
      j = close < 0 ? j + n : close + n;
    } else if (c === 93) {
      res = j;
      break;
    } else {
      j++;
    }
  }
  sc.bracket = { from: p, res };
  return res;
}

function findParenClose(sc: Scan, p: number): number {
  const hit = cached(sc.paren, p);
  if (hit !== undefined) return hit;
  let res = sc.s.indexOf(")", p);
  if (res >= sc.hi) res = -1;
  sc.paren = { from: p, res };
  return res;
}

/** Recognise `[text](url)` starting at `open`. */
function tryLink(sc: Scan, open: number): { textEnd: number; end: number; url: string } | null {
  const textEnd = findBracketClose(sc, open + 1);
  if (textEnd < 0 || sc.s.charCodeAt(textEnd + 1) !== 40 || textEnd + 1 >= sc.hi) return null;
  const urlEnd = findParenClose(sc, textEnd + 2);
  if (urlEnd < 0) return null;
  return { textEnd, end: urlEnd + 1, url: sc.s.slice(textEnd + 2, urlEnd).trim() };
}

function decodeUrl(url: string): string {
  try {
    return decodeURI(url);
  } catch {
    return url;
  }
}
