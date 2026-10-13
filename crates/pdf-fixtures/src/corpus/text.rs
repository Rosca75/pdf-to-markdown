//! Text-centric corpus cases: structure, lists, emphasis, links, specials, code,
//! kerning, overprint, encodings and ligatures.

use super::common::*;
use crate::content::{adj, t, Content};
use crate::fonts::{Std14, TextFont};
use crate::layout::{put_lines, wrap, StdFonts};
use crate::writer::{PageSpec, Pdf};

pub fn basic_structure() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 24.0, "Annual Report");
    p.body(
        &f.times,
        "This report summarises the financial performance of the company during the last fiscal year and outlines the priorities for the coming twelve months. It is intended for shareholders and the executive board.",
    );
    p.heading(&f.helv_b, 16.0, "Market overview");
    p.body(
        &f.times,
        "The overall market grew by four percent, driven mainly by strong demand in the industrial segment. Consumer demand remained flat, and competitors reacted with aggressive discounts in the second half of the year.",
    );
    p.body(
        &f.times,
        "Our own share of the market increased slightly. Management attributes this result to the new product line and to a more focused sales organisation.",
    );
    p.heading(&f.helv_b, 13.0, "Regional results");
    p.body(
        &f.times,
        "Europe delivered the highest revenue growth of all regions, while North America contributed the largest absolute profit. Asia remains a long term investment with modest short term returns.",
    );
    p.heading(&f.helv_b, 16.0, "Outlook");
    p.body(
        &f.times,
        "For the coming year we expect stable growth and continued investment in automation. The board proposes an unchanged dividend.",
    );
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}

pub fn lists() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let u = pdf.add_unicode_font("ArialMT");
    let mut c = Content::new();
    let size = 11.0;
    let lead = 14.0;
    let mut y = TOP;
    c.text(
        &f.times,
        size,
        72.0,
        y,
        "Checklist for the project kickoff:",
    );
    y -= 22.0;

    // One list item: marker at `mx`, wrapped text at `tx` with hanging indent.
    let item = |c: &mut Content,
                mfont: &dyn TextFont,
                marker: &str,
                mx: f64,
                tx: f64,
                text: &str,
                y: &mut f64| {
        c.text(mfont, size, mx, *y, marker);
        let lines = wrap(&f.times, size, text, 540.0 - tx);
        *y = put_lines(c, &f.times, size, tx, *y, lead, &lines, None) - 3.0;
    };

    item(&mut c, &f.times, "\u{2022}", 72.0, 90.0, "Prepare the budget forecast for the next quarter, including all regional adjustments and the revised headcount plan.", &mut y);
    item(
        &mut c,
        &f.times,
        "\u{2022}",
        72.0,
        90.0,
        "Book the meeting room",
        &mut y,
    );
    item(
        &mut c,
        &u,
        "\u{25E6}",
        96.0,
        114.0,
        "Check projector availability and bring the spare cable for the presentation.",
        &mut y,
    );
    item(&mut c, &u, "\u{25E6}", 96.0, 114.0, "Order coffee", &mut y);
    item(
        &mut c,
        &f.times,
        "\u{2022}",
        72.0,
        90.0,
        "Send the invitations",
        &mut y,
    );
    y -= 10.0;
    item(
        &mut c,
        &f.times,
        "\u{2013}",
        72.0,
        90.0,
        "dash item one",
        &mut y,
    );
    item(&mut c, &f.times, "\u{2013}", 72.0, 90.0, "dash item two with a long continuation that definitely needs to wrap onto a second line of the page", &mut y);
    y -= 10.0;
    c.text(&f.times, size, 72.0, y, "Installation steps:");
    y -= 20.0;
    item(&mut c, &f.times, "1.", 72.0, 94.0, "Open the settings dialog and choose the advanced tab where the export options are located.", &mut y);
    item(
        &mut c,
        &f.times,
        "2.",
        72.0,
        94.0,
        "Select the output folder",
        &mut y,
    );
    item(
        &mut c,
        &f.times,
        "3.",
        72.0,
        94.0,
        "Press the convert button",
        &mut y,
    );
    y -= 10.0;
    c.text(&f.times, size, 72.0, y, "Other numbering styles:");
    y -= 20.0;
    item(
        &mut c,
        &f.times,
        "1)",
        72.0,
        94.0,
        "Alpha point of the parenthesis list",
        &mut y,
    );
    item(
        &mut c,
        &f.times,
        "2)",
        72.0,
        94.0,
        "Beta point of the parenthesis list",
        &mut y,
    );
    item(
        &mut c,
        &f.times,
        "a.",
        72.0,
        94.0,
        "First lettered sub-step",
        &mut y,
    );
    item(
        &mut c,
        &f.times,
        "b.",
        72.0,
        94.0,
        "Second lettered sub-step",
        &mut y,
    );
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn emphasis() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut c = Content::new();
    let s = 12.0;
    let mut y = TOP;
    runs(
        &mut c,
        s,
        72.0,
        y,
        &[
            (&f.times, "This sentence has "),
            (&f.times_b, "bold"),
            (&f.times, ", "),
            (&f.times_i, "italic"),
            (&f.times, " and "),
            (&f.times_bi, "bold italic"),
            (&f.times, " words inside it."),
        ],
    );
    y -= 30.0;
    runs(
        &mut c,
        s,
        72.0,
        y,
        &[
            (&f.times_b, "Important: "),
            (&f.times, "read the terms before continuing."),
        ],
    );
    y -= 30.0;
    runs(
        &mut c,
        s,
        72.0,
        y,
        &[
            (&f.times, "A "),
            (&f.times_b, "very important"),
            (&f.times, " notice, "),
            (&f.times_i, "not so much"),
            (&f.times, "."),
        ],
    );
    y -= 30.0;
    runs(
        &mut c,
        s,
        72.0,
        y,
        &[
            (&f.helv, "Sans text with "),
            (&f.helv_i, "oblique emphasis"),
            (&f.helv, " in the middle."),
        ],
    );
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn links() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut c = Content::new();
    let s = 12.0;
    let mut annots = Vec::new();

    // Single Tj, link over one word.
    let mut y = TOP;
    let line = "Read the documentation for all the details.";
    c.text(&f.times, s, 72.0, y, line);
    let x0 = 72.0 + f.times.width("Read the ", s);
    let x1 = x0 + f.times.width("documentation", s);
    annots.push(pdf.add_link([x0, y - 3.0, x1, y + 9.0], "https://example.com/docs"));

    // Separate runs, link over two words.
    y -= 30.0;
    let end = runs(
        &mut c,
        s,
        72.0,
        y,
        &[
            (&f.times, "Visit "),
            (&f.times, "our website"),
            (&f.times, " today."),
        ],
    );
    let _ = end;
    let lx0 = 72.0 + f.times.width("Visit ", s);
    let lx1 = lx0 + f.times.width("our website", s);
    annots.push(pdf.add_link([lx0, y - 3.0, lx1, y + 9.0], "https://example.org/"));

    // A wrapped paragraph with a link on a word in its second line.
    y -= 30.0;
    let text = "For installation instructions please consult the setup guide on the support portal before you start the installation of the server software on your machine.";
    let lines = wrap(&f.times, s, text, 300.0);
    put_lines(&mut c, &f.times, s, 72.0, y, 15.0, &lines, None);
    let (li, word) = lines
        .iter()
        .enumerate()
        .find_map(|(i, l)| l.contains("portal").then_some((i, "portal")))
        .expect("word on some line");
    let l = &lines[li];
    let pre = &l[..l.find(word).unwrap_or(0)];
    let px0 = 72.0 + f.times.width(pre, s);
    let px1 = px0 + f.times.width(word, s);
    let py = y - 15.0 * li as f64;
    annots.push(pdf.add_link(
        [px0, py - 3.0, px1, py + 9.0],
        "https://example.net/support",
    ));

    // Bare URL text with a matching annotation.
    y -= 100.0;
    let url = "https://example.net/page";
    c.text(&f.times, s, 72.0, y, url);
    annots.push(pdf.add_link([72.0, y - 3.0, 72.0 + f.times.width(url, s), y + 9.0], url));

    pdf.add_page(PageSpec::letter().with_annots(annots), &c);
    finish(&mut pdf)
}

pub fn markdown_specials() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 18.0, "Special characters");
    p.body(
        &f.times,
        "Use * and _ carefully: 2 * 3 = 6 and snake_case_name should survive. Brackets [like this] and `backticks` need care too, and so does a pipe | between words.",
    );
    p.body(
        &f.times,
        "Angle <tags> and comparisons such as 5 > 3 appear as well.",
    );
    p.body(
        &f.times,
        "# Not a heading but a paragraph that merely starts with a hash sign.",
    );
    p.body(
        &f.times,
        "1984. That year was long ago and the number must not turn into a list.",
    );
    p.body(&f.times, "Final paragraph without anything special.");
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}

pub fn code_block() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 18.0, "Example");
    p.body(
        &f.times,
        "The following program prints a number when it is large enough:",
    );
    p.skip(2.0);
    p.lines(
        &f.cour,
        10.0,
        &[
            "fn main() {",
            "    let x = 42;",
            "    if x > 1 {",
            "        println!(\"{}\", x);",
            "    }",
            "}",
        ],
    );
    p.body(
        &f.times,
        "After the listing the normal text continues here.",
    );
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}

pub fn title_page() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut c = Content::new();
    c.text(&f.helv_b, 40.0, 72.0, 520.0, "Market Study 2026");
    c.text(
        &f.helv,
        20.0,
        72.0,
        470.0,
        "Prospects for the Mid-Size Segment",
    );
    c.text(&f.times, 12.0, 72.0, 400.0, "Prepared by Jane Doe");
    pdf.add_page(PageSpec::letter(), &c);

    let mut p = Page::new();
    p.heading(&f.helv_b, 16.0, "Executive summary");
    p.body(
        &f.times,
        "The mid-size segment is expected to outgrow the overall market over the next three years. This study explains why and identifies the most attractive sub-segments.",
    );
    p.body(
        &f.times,
        "Our analysis is based on interviews with forty industry experts and on a review of public filings.",
    );
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}

pub fn tj_kerning() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut c = Content::new();
    c.text(
        &f.times,
        12.0,
        72.0,
        720.0,
        "Normal paragraph before the kerned lines.",
    );

    c.begin_text();
    c.set_font(&f.times, 12.0);
    c.td(72.0, 690.0);
    c.show_adjusted(
        &f.times,
        &[
            t("Hello"),
            adj(-280.0),
            t("world"),
            adj(-280.0),
            t("from"),
            adj(-280.0),
            t("kerning"),
            adj(-280.0),
            t("arrays."),
        ],
    );
    c.end_text();

    c.begin_text();
    c.set_font(&f.times, 12.0);
    c.td(72.0, 660.0);
    c.show_adjusted(
        &f.times,
        &[
            t("W"),
            adj(80.0),
            t("ARNING"),
            adj(-250.0),
            t("do"),
            adj(-250.0),
            t("not"),
            adj(-250.0),
            t("touch"),
        ],
    );
    c.end_text();

    // Glyph by glyph with individual Td moves; word gaps are pure movement.
    c.begin_text();
    c.set_font(&f.times, 12.0);
    c.td(72.0, 630.0);
    for word in ["Glyph", "by", "glyph", "text"] {
        for ch in word.chars() {
            let s = ch.to_string();
            c.show(&f.times, &s);
            c.td(f.times.width(&s, 12.0), 0.0);
        }
        c.td(3.0, 0.0);
    }
    c.end_text();

    c.text(
        &f.times,
        12.0,
        72.0,
        600.0,
        "Normal paragraph after the kerned lines.",
    );
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn fake_bold_overprint() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut c = Content::new();
    for dx in [0.0, 0.3] {
        c.text(&f.helv, 18.0, 72.0 + dx, 700.0, "Fake Bold Heading");
    }
    for dx in [0.0, 0.3] {
        c.text(
            &f.times,
            11.0,
            72.0 + dx,
            660.0,
            "This sentence is overprinted to look bold.",
        );
    }
    c.text(
        &f.times,
        11.0,
        72.0,
        630.0,
        "This sentence is printed normally, once.",
    );
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn encodings() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let diff = pdf.add_std_font(
        Std14::Helvetica,
        &[
            (1, "alpha"),
            (2, "beta"),
            (3, "gamma"),
            (4, "uni0416"),
            (5, "uni0436"),
            (6, "Euro"),
            (0x5E, "emdash"),
        ],
    );
    let uf = pdf.add_unicode_font("ArialMT");
    let mut c = Content::new();
    let s = 12.0;
    c.text(
        &f.helv,
        s,
        72.0,
        720.0,
        "Caf\u{E9} r\u{E9}sum\u{E9} na\u{EF}ve Zo\u{EB} M\u{FC}ller \u{C5}ngstr\u{F6}m costs \u{20AC}25 \u{2014} \u{201C}quoted\u{201D} and \u{2018}single\u{2019} with en\u{2013}dash and ellipsis\u{2026}",
    );
    c.text(
        &diff,
        s,
        72.0,
        690.0,
        "Differences: \u{3B1}\u{3B2}\u{3B3} \u{416}\u{436} \u{20AC} and A\u{2014}B ok",
    );
    c.text(
        &uf,
        s,
        72.0,
        660.0,
        "\u{395}\u{3BB}\u{3BB}\u{3B7}\u{3BD}\u{3B9}\u{3BA}\u{3AC}",
    );
    c.text(
        &uf,
        s,
        72.0,
        640.0,
        "\u{41F}\u{440}\u{438}\u{432}\u{435}\u{442} \u{43C}\u{438}\u{440}",
    );
    c.text(
        &uf,
        s,
        72.0,
        620.0,
        "\u{65E5}\u{672C}\u{8A9E}\u{306E}\u{30C6}\u{30AD}\u{30B9}\u{30C8}",
    );
    c.text(&uf, s, 72.0, 600.0, "\u{4E2D}\u{6587}\u{6D4B}\u{8BD5}");
    c.text(
        &uf,
        s,
        72.0,
        580.0,
        "Mixed Latin and \u{391}\u{392}\u{393} in one font.",
    );
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn ligatures() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let lig = pdf.add_std_font(Std14::Helvetica, &[(1, "fi"), (2, "fl")]);
    let uf = pdf.add_unicode_font("ArialMT");
    let mut c = Content::new();
    c.text(
        &uf,
        12.0,
        72.0,
        720.0,
        "The \u{FB01}nal \u{FB02}ow is ef\u{FB01}cient",
    );
    c.text(&uf, 12.0, 72.0, 700.0, "The o\u{FB03}ce opens at nine");
    c.text(
        &lig,
        12.0,
        72.0,
        680.0,
        "Second: \u{FB01}nal \u{FB02}ow of ef\u{FB01}cient staff",
    );
    c.text(&f.helv, 12.0, 72.0, 660.0, "Plain fi and fl for comparison");
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}
