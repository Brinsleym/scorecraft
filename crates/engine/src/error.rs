use std::fmt;

/// Every way a command can fail. Commands return these; they never panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// No command is registered under this id.
    UnknownCommand(String),
    /// The command exists but is disabled in the current state (greyed out in menus).
    Disabled(String),
    /// The params failed validation; the message says which field and why.
    BadParams { command: String, message: String },
    /// The command or an option of it is not implemented yet.
    Unsupported(String),
    /// The score could not be opened, rendered or read; carries the PDF layer's stable code.
    Pdf(scorecraft_pdf::PdfError),
    /// A file could not be read or written; the message names the path and the reason.
    Io(String),
    /// A library edit was refused (missing record, out of range, would break integrity); carries
    /// the ops layer's stable code.
    Edit(scorecraft_ops::OpError),
    /// The library store failed (unreadable, too new, not a library); carries its stable code.
    Store(scorecraft_format::StoreError),
    /// Pen samples could not make a stroke (none usable, too many, far off the page); carries the
    /// ink layer's stable code.
    Ink(scorecraft_ink::InkError),
}

impl EngineError {
    /// A stable machine-readable code for the control channel and MCP (`docs/control-protocol.md`).
    pub fn code(&self) -> &'static str {
        match self {
            EngineError::UnknownCommand(_) => "unknown_command",
            EngineError::Disabled(_) => "disabled",
            EngineError::BadParams { .. } => "bad_params",
            EngineError::Unsupported(_) => "unsupported",
            EngineError::Pdf(e) => e.code(),
            EngineError::Io(_) => "io_error",
            EngineError::Edit(e) => e.code(),
            EngineError::Store(e) => e.code(),
            EngineError::Ink(e) => e.code(),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::UnknownCommand(id) => write!(f, "unknown command `{id}`"),
            EngineError::Disabled(id) => write!(f, "command `{id}` is not available right now"),
            EngineError::BadParams { command, message } => {
                write!(f, "bad params for `{command}`: {message}")
            }
            EngineError::Unsupported(what) => write!(f, "not supported yet: {what}"),
            EngineError::Pdf(e) => e.fmt(f),
            EngineError::Io(why) => f.write_str(why),
            EngineError::Edit(e) => e.fmt(f),
            EngineError::Store(e) => e.fmt(f),
            EngineError::Ink(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<scorecraft_pdf::PdfError> for EngineError {
    fn from(e: scorecraft_pdf::PdfError) -> Self {
        EngineError::Pdf(e)
    }
}

impl From<scorecraft_ops::OpError> for EngineError {
    fn from(e: scorecraft_ops::OpError) -> Self {
        EngineError::Edit(e)
    }
}

impl From<scorecraft_format::StoreError> for EngineError {
    fn from(e: scorecraft_format::StoreError) -> Self {
        EngineError::Store(e)
    }
}

impl From<scorecraft_ink::InkError> for EngineError {
    fn from(e: scorecraft_ink::InkError) -> Self {
        EngineError::Ink(e)
    }
}
