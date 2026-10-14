//! Typed conversion errors with stable codes and user-facing wording.

use std::fmt;

/// Why a document could not be converted.
///
/// Every variant maps to a stable machine code (`code`) that the UI keys on,
/// a one-sentence explanation (`message`) and an actionable next step (`hint`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    NotFound,
    PermissionDenied,
    NotAPdf,
    Corrupt(String),
    PasswordRequired,
    WrongPassword,
    NoTextLayer,
    Cancelled,
    Io(String),
}

impl ConvertError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::PermissionDenied => "permission_denied",
            Self::NotAPdf => "not_a_pdf",
            Self::Corrupt(_) => "corrupt",
            Self::PasswordRequired => "password_required",
            Self::WrongPassword => "wrong_password",
            Self::NoTextLayer => "no_text_layer",
            Self::Cancelled => "cancelled",
            Self::Io(_) => "io",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "The file no longer exists at this location.".into(),
            Self::PermissionDenied => "The file cannot be read: access was denied.".into(),
            Self::NotAPdf => "This file is not a PDF document.".into(),
            Self::Corrupt(detail) => {
                format!("The PDF is damaged and could not be read ({detail}).")
            }
            Self::PasswordRequired => "This PDF is protected by a password.".into(),
            Self::WrongPassword => "The password is incorrect.".into(),
            Self::NoTextLayer => {
                "This PDF contains no extractable text (it is probably a scanned image).".into()
            }
            Self::Cancelled => "Conversion was cancelled.".into(),
            Self::Io(detail) => format!("The file could not be read ({detail})."),
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            Self::NotFound => "Check that the file was not moved or deleted, then add it again.",
            Self::PermissionDenied => "Copy the file to a folder you own (for example Documents) and add it again.",
            Self::NotAPdf => "Choose a file with the .pdf extension. Renaming another format to .pdf does not convert it.",
            Self::Corrupt(_) => "Open the file in a PDF viewer and re-save or print it to a new PDF, then try again.",
            Self::PasswordRequired => "Enter the document password to unlock it.",
            Self::WrongPassword => "Check the password (it is case-sensitive) and try again.",
            Self::NoTextLayer => "Run the PDF through an OCR tool first; OCR is not supported by PDF2MD.",
            Self::Cancelled => "Add the file again to restart the conversion.",
            Self::Io(_) => "Make sure the file is not open in another program and the drive is available.",
        }
    }
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ConvertError {}

impl From<std::io::Error> for ConvertError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            _ => Self::Io(e.to_string()),
        }
    }
}
