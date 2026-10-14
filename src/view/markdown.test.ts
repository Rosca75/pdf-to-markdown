import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

function render(md: string): HTMLDivElement {
  const div = document.createElement("div");
  div.appendChild(renderMarkdown(md));
  return div;
}

function html(md: string): string {
  return render(md).innerHTML;
}

describe("renderMarkdown blocks", () => {
  it("renders nothing for empty input", () => {
    expect(renderMarkdown("").childNodes.length).toBe(0);
    expect(renderMarkdown("\n\n").childNodes.length).toBe(0);
  });

  it("renders a heading followed by a paragraph", () => {
    expect(html("## Market overview\n\nWidgets sell.\n")).toBe(
      "<h2>Market overview</h2><p>Widgets sell.</p>",
    );
  });

  it("renders heading levels 1 to 6", () => {
    for (let level = 1; level <= 6; level++) {
      expect(html(`${"#".repeat(level)} Title\n`)).toBe(`<h${level}>Title</h${level}>`);
    }
    expect(html("####### seven\n")).toBe("<p>####### seven</p>");
  });

  it("keeps an escaped trailing hash in a heading", () => {
    expect(html("## Issue \\#\n")).toBe("<h2>Issue #</h2>");
    expect(html("## C\\#\n")).toBe("<h2>C#</h2>");
  });

  it("joins consecutive lines into one paragraph", () => {
    expect(html("one\ntwo\n\nthree\n")).toBe("<p>one two</p><p>three</p>");
  });

  it("renders tight nested lists and switches list type on marker change", () => {
    const d = render("Intro\n\n- one\n  - nested\n3. three\n1) paren\n\nAfter\n");
    expect(d.innerHTML).toBe(
      "<p>Intro</p>" +
        "<ul><li>one<ul><li>nested</li></ul></li></ul>" +
        '<ol start="3"><li>three</li></ol>' +
        "<ol><li>paren</li></ol>" +
        "<p>After</p>",
    );
  });

  it("sets start only when the first number is not 1", () => {
    expect(html("1. a\n2. b\n")).toBe("<ol><li>a</li><li>b</li></ol>");
    expect(html("7. a\n")).toBe('<ol start="7"><li>a</li></ol>');
  });

  it("nests ordered items under bullets and back", () => {
    expect(html("- a\n   1. x\n   2. y\n- b\n")).toBe(
      "<ul><li>a<ol><li>x</li><li>y</li></ol></li><li>b</li></ul>",
    );
    expect(html("1. a\n   - x\n      1. deep\n2. b\n")).toBe(
      "<ol><li>a<ul><li>x<ol><li>deep</li></ol></li></ul></li><li>b</li></ol>",
    );
  });

  it("keeps escaped block starts as paragraphs", () => {
    expect(html("\\# not a heading\n")).toBe("<p># not a heading</p>");
    expect(html("1984\\. That year\n")).toBe("<p>1984. That year</p>");
    expect(html("\\- not a list\n")).toBe("<p>- not a list</p>");
    expect(html("\\---\n")).toBe("<p>---</p>");
    expect(html("-5 degrees\n")).toBe("<p>-5 degrees</p>");
    expect(html("3.5 percent\n")).toBe("<p>3.5 percent</p>");
    expect(html("- 1\\. not nested\n")).toBe("<ul><li>1. not nested</li></ul>");
    expect(html("\\= x \\+ y\n")).toBe("<p>= x + y</p>");
  });
});

describe("renderMarkdown code", () => {
  it("renders a fenced block with textContent", () => {
    const d = render("```\nlet a = 1;\n  <b>x</b>\n```\n");
    expect(d.querySelector("pre > code")?.textContent).toBe("let a = 1;\n  <b>x</b>");
    expect(d.querySelector("b")).toBeNull();
  });

  it("handles a longer fence containing backticks", () => {
    const d = render("````\nlet a = 1;\n  ```inner\n````\n\nafter\n");
    expect(d.querySelector("code")?.textContent).toBe("let a = 1;\n  ```inner");
    expect(d.querySelectorAll("p").length).toBe(1);
  });

  it("runs an unclosed fence to the end of the document", () => {
    const d = render("para\n\n```\ncode\n\n# not heading\n");
    expect(d.querySelector("code")?.textContent).toBe("code\n\n# not heading");
    expect(d.querySelector("h1")).toBeNull();
  });

  it("renders inline code spans", () => {
    expect(html("Run `cargo *test*`\n")).toBe("<p>Run <code>cargo *test*</code></p>");
    expect(html("a ``b ` c`` d\n")).toBe("<p>a <code>b ` c</code> d</p>");
    expect(html("`` `x` ``\n")).toBe("<p><code>`x`</code></p>");
    expect(html("`  `\n")).toBe("<p><code>  </code></p>");
  });
});

describe("renderMarkdown tables", () => {
  it("renders header, body, escaped pipes and padded short rows", () => {
    const d = render("| Name | Value |\n| --- | --- |\n| a\\|b | 1 |\n| short |  |\n");
    expect(d.querySelectorAll("thead th").length).toBe(2);
    const rows = d.querySelectorAll("tbody tr");
    expect(rows.length).toBe(2);
    expect(rows[0].children[0].textContent).toBe("a|b");
    expect(rows[1].children.length).toBe(2);
    expect(rows[1].children[1].textContent).toBe("");
  });

  it("pads rows that have fewer cells than the header", () => {
    const d = render("| A | B | C |\n| --- | --- | --- |\n| 1 |\n");
    expect(d.querySelectorAll("tbody td").length).toBe(3);
  });

  it("renders code and emphasis in cells, unescaping pipes inside code", () => {
    const d = render("| H |\n| --- |\n| `a\\|b` **x** |\n");
    expect(d.querySelector("td code")?.textContent).toBe("a|b");
    expect(d.querySelector("td strong")?.textContent).toBe("x");
  });

  it("keeps an escaped backslash before a pipe as a separator", () => {
    const d = render("| a\\\\ | b |\n| --- | --- |\n");
    expect(d.querySelectorAll("th").length).toBe(2);
    expect(d.querySelector("th")?.textContent).toBe("a\\");
  });

  it("keeps a backslash before a pipe from the engine's plain-text fallback", () => {
    const d = render("| h |\n| --- |\n| a\\\\\\|b |\n");
    expect(d.querySelectorAll("td").length).toBe(1);
    expect(d.querySelector("td")?.textContent).toBe("a\\|b");
  });

  it("treats a pipe line without a delimiter row as a paragraph", () => {
    expect(html("| just text\n")).toBe("<p>| just text</p>");
  });

  it("ends a table at the next blank line", () => {
    const d = render("| H |\n| --- |\n| x |\n\nafter\n");
    expect(d.querySelectorAll("tbody tr").length).toBe(1);
    expect(d.querySelectorAll("p").length).toBe(1);
  });
});

describe("renderMarkdown inlines", () => {
  it("renders strong, em and both", () => {
    expect(html("This is **bold** and *italic* and ***both***.\n")).toBe(
      "<p>This is <strong>bold</strong> and <em>italic</em> and <strong><em>both</em></strong>.</p>",
    );
  });

  it("splits glued runs of different emphasis", () => {
    expect(html("**a***b*\n")).toBe("<p><strong>a</strong><em>b</em></p>");
    expect(html("*a***b**\n")).toBe("<p><em>a</em><strong>b</strong></p>");
    expect(html("**a*****b***\n")).toBe("<p><strong>a</strong><strong><em>b</em></strong></p>");
    expect(html("*a****b***\n")).toBe("<p><em>a</em><strong><em>b</em></strong></p>");
  });

  it("renders the special-characters escape case literally", () => {
    const d = render("a\\*b \\[c\\] \\`d\\` \\<e\\> \\~f snake_case \\_x\\_ back\\\\slash\n");
    expect(d.textContent).toBe("a*b [c] `d` <e> ~f snake_case _x_ back\\slash");
    expect(d.querySelector("em, code, strong, a")).toBeNull();
  });

  it("nests code and escapes inside emphasis", () => {
    expect(html("*a `b*c` \\* d*\n")).toBe("<p><em>a <code>b*c</code> * d</em></p>");
  });

  it("keeps unmatched markers literal", () => {
    expect(html("a * b * c\n")).toBe("<p>a * b * c</p>");
    expect(html("*open only\n")).toBe("<p>*open only</p>");
    expect(html("[open only\n")).toBe("<p>[open only</p>");
    expect(html("[a](b\n")).toBe("<p>[a](b</p>");
    expect(html("[a] (b)\n")).toBe("<p>[a] (b)</p>");
    expect(html("tick ` alone\n")).toBe("<p>tick ` alone</p>");
    expect(html("****\n")).toBe("<p>****</p>");
  });

  it("renders links without href and with a decoded title", () => {
    const d = render("Visit [example.com](https://example.com/a%20b) now.\n");
    const a = d.querySelector("a") as HTMLAnchorElement;
    expect(a.hasAttribute("href")).toBe(false);
    expect(a.className).toBe("md-link");
    expect(a.getAttribute("title")).toBe("https://example.com/a b");
    expect(a.textContent).toBe("example.com");
    expect(d.textContent).toBe("Visit example.com now.");
  });

  it("renders emphasis and code inside links, and falls back on bad escapes", () => {
    const d = render("[**bold** `c`](https://x.test/%E0%A4%A)\n");
    const a = d.querySelector("a") as HTMLElement;
    expect(a.querySelector("strong")?.textContent).toBe("bold");
    expect(a.querySelector("code")?.textContent).toBe("c");
    expect(a.getAttribute("title")).toBe("https://x.test/%E0%A4%A");
  });

  it("renders links inside table cells", () => {
    const d = render("| H |\n| --- |\n| [go](https://x.test/a%7Cb%20c%5Cd) |\n");
    expect(d.querySelector("td a")?.getAttribute("title")).toBe("https://x.test/a|b c\\d");
  });
});

describe("renderMarkdown security", () => {
  it("renders raw script tags as literal text", () => {
    const d = render("<script>alert(1)</script>\n\n\\<script\\>alert(1)\\</script\\>\n");
    expect(d.querySelector("script")).toBeNull();
    const ps = d.querySelectorAll("p");
    expect(ps[0].textContent).toBe("<script>alert(1)</script>");
    expect(ps[1].textContent).toBe("<script>alert(1)</script>");
  });

  it("never emits href, src or event attributes", () => {
    const d = render(
      "[x](javascript:alert(1))\n\n<img src=x onerror=alert(1)>\n\n| <b onclick=1> |\n| --- |\n| [y](data:text/html,x) |\n\n```\n<a href=x>\n```\n",
    );
    expect(d.querySelectorAll("img, b, script").length).toBe(0);
    for (const el of Array.from(d.querySelectorAll("*"))) {
      expect(el.hasAttribute("href")).toBe(false);
      expect(el.hasAttribute("src")).toBe(false);
      expect(el.getAttributeNames().some((n) => n.startsWith("on"))).toBe(false);
    }
    expect(d.querySelector("a")?.getAttribute("title")).toBe("javascript:alert(1");
  });
});

describe("renderMarkdown performance", () => {
  it("renders a 2,000,000 character document quickly", () => {
    const chunk =
      "## Heading\n\nSome **bold** and *italic* text with `code` and [link](https://x.test/a%20b) end.\n\n" +
      "- one\n  - nested *x*\n- two\n3. three\n\n" +
      "| A | B |\n| --- | --- |\n| a\\|b | `c` |\n| short |  |\n\n" +
      "```\ncode line\n```\n\n";
    const md = chunk.repeat(Math.ceil(2_000_000 / chunk.length));
    expect(md.length).toBeGreaterThanOrEqual(2_000_000);
    const start = performance.now();
    const frag = renderMarkdown(md);
    expect(performance.now() - start).toBeLessThan(5000);
    expect(frag.childNodes.length).toBeGreaterThan(1000);
  });

  it("handles a pathological line of unmatched markers", () => {
    for (const unit of ["*", "`", "[", "*a **b ", "[a](", "`a ``b ", "[a ]("]) {
      const md = unit.repeat(Math.ceil(100_000 / unit.length)) + "\n";
      const start = performance.now();
      renderMarkdown(md);
      expect(performance.now() - start).toBeLessThan(2000);
    }
  });
});
