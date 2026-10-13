/** Tiny DOM helpers. All text goes through textContent, never innerHTML. */

type Attrs = Record<string, string | boolean | undefined>;

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === false) continue;
    node.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children) node.append(c);
  return node;
}

export function setText(node: Element, text: string): void {
  if (node.textContent !== text) node.textContent = text;
}

export function setHidden(node: HTMLElement, hidden: boolean): void {
  if (node.hidden !== hidden) node.hidden = hidden;
}

export function setAttr(node: Element, name: string, value: string | null): void {
  if (value === null) {
    if (node.hasAttribute(name)) node.removeAttribute(name);
  } else if (node.getAttribute(name) !== value) {
    node.setAttribute(name, value);
  }
}

export function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

export function mustFind<T extends Element>(root: ParentNode, selector: string): T {
  const found = root.querySelector<T>(selector);
  if (!found) throw new Error(`Missing element: ${selector}`);
  return found;
}
