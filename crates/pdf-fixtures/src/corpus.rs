//! The PDF2MD test corpus. Every case is generated deterministically.

mod common;
mod graphics;
mod pages;
mod special;
mod tables;
mod text;

/// Every corpus file as `(file name, bytes)`.
pub fn cases() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("basic_structure.pdf", text::basic_structure()),
        ("lists.pdf", text::lists()),
        ("emphasis.pdf", text::emphasis()),
        ("table_ruled.pdf", tables::table_ruled()),
        ("table_unruled.pdf", tables::table_unruled()),
        ("links.pdf", text::links()),
        ("headers_footers.pdf", pages::headers_footers()),
        ("margin_jumps.pdf", pages::margin_jumps()),
        ("split_across_pages.pdf", pages::split_across_pages()),
        ("two_columns.pdf", pages::two_columns()),
        ("mixed_graphics.pdf", graphics::mixed_graphics()),
        ("encodings.pdf", text::encodings()),
        ("ligatures.pdf", text::ligatures()),
        ("markdown_specials.pdf", text::markdown_specials()),
        ("blank_pages.pdf", pages::blank_pages()),
        ("scanned_image_only.pdf", graphics::scanned_image_only()),
        ("ocr_invisible_text.pdf", graphics::ocr_invisible_text()),
        (
            "encrypted_user_password.pdf",
            special::encrypted_user_password(),
        ),
        ("encrypted_owner_only.pdf", special::encrypted_owner_only()),
        ("encrypted_aes128.pdf", special::encrypted_aes128()),
        ("malformed_bad_xref.pdf", special::malformed_bad_xref()),
        ("malformed_truncated.pdf", special::malformed_truncated()),
        (
            "malformed_garbage_prefix.pdf",
            special::malformed_garbage_prefix(),
        ),
        ("malformed_no_eof.pdf", special::malformed_no_eof()),
        ("not_a_pdf.pdf", special::not_a_pdf()),
        ("zero_bytes.pdf", special::zero_bytes()),
        ("corrupt_stream.pdf", special::corrupt_stream()),
        ("tj_kerning.pdf", text::tj_kerning()),
        ("rotated_margin_text.pdf", pages::rotated_margin_text()),
        ("object_streams.pdf", special::object_streams()),
        ("incremental_update.pdf", special::incremental_update()),
        ("code_block.pdf", text::code_block()),
        ("fake_bold_overprint.pdf", text::fake_bold_overprint()),
        ("title_page.pdf", text::title_page()),
    ]
}
