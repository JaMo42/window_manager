use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use std::{
    io,
    ops::{Deref, DerefMut, Range},
};

struct ErrorWriter<'a>(pub &'a mut Vec<u8>);

impl<'a> io::Write for ErrorWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// This type is quit big but we don't want to box the `Error` type everywhere
// so we hide the actual data in a boxed value inside the error.
// Elements can still be accessed normally using the deref traits.
#[derive(Default)]
pub struct ErrorInner {
    pub(super) message: String,
    pub(super) line: Option<usize>,
    pub(super) label: Option<(Range<usize>, String)>,
    pub(super) why: Option<(Range<usize>, String)>,
    pub(super) help: Option<String>,
    pub(super) note: Option<String>,
}

#[derive(Default)]
pub struct Error {
    inner: Box<ErrorInner>,
}

impl Deref for Error {
    type Target = ErrorInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Error {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Error {
    pub fn new(message: impl ToString) -> Self {
        Self {
            inner: Box::new(ErrorInner {
                message: message.to_string(),
                ..Default::default()
            }),
        }
    }

    /// Creates a new error on the line of the given location.
    pub fn new_with_line(message: impl ToString, line: usize) -> Self {
        Self {
            inner: Box::new(ErrorInner {
                message: message.to_string(),
                line: Some(line),
                ..Default::default()
            }),
        }
    }

    #[allow(dead_code)]
    pub fn with_message(mut self, message: impl ToString) -> Self {
        self.message = message.to_string();
        self
    }

    pub fn with_label(mut self, span: Range<usize>, message: impl ToString) -> Self {
        self.label = Some((span, message.to_string()));
        self
    }

    pub fn with_why(mut self, span: Range<usize>, message: impl ToString) -> Self {
        self.why = Some((span, message.to_string()));
        self
    }

    pub fn with_help(mut self, help: impl ToString) -> Self {
        self.help = Some(help.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn with_note(mut self, note: impl ToString) -> Self {
        self.help = Some(note.to_string());
        self
    }

    pub fn to_markup(&self, path: &str, source: &str) -> String {
        let line = self.line.expect("error has no line information");
        let mut builder = Report::build(ReportKind::Error, path, line).with_message(&self.message);
        if let Some((span, msg)) = self.label.clone() {
            builder.add_label(
                Label::new((path, span))
                    .with_color(Color::Red)
                    .with_message(msg.as_str().fg(Color::Red)),
            );
        }
        if let Some((span, msg)) = self.why.clone() {
            builder.add_label(
                Label::new((path, span))
                    .with_color(Color::Blue)
                    .with_message(msg.fg(Color::Blue)),
            );
        }
        if let Some(msg) = &self.help {
            builder.set_help(msg);
        }
        if let Some(msg) = &self.note {
            builder.set_note(msg);
        }
        let report = builder.finish();
        let mut buf = Vec::new();
        let writer = ErrorWriter(&mut buf);
        report.write((path, Source::from(source)), writer).unwrap();
        let ansi_message = String::from_utf8(buf).unwrap();
        let message = ansi_message
            .replace("\x1b[0m", "</span>")
            .replace("\x1b[31m", "<span fgcolor='tomato'>")
            .replace("\x1b[33m", "<span fgcolor='gold'>")
            .replace("\x1b[34m", "<span fgcolor='cornflowerblue'>")
            .replace("\x1b[38;5;115m", "<span fgcolor='darkseagreen'>")
            .replace("\x1b[38;5;246m", "<span fgcolor='grey'>")
            .replace("\x1b[38;5;249m", "<span>");
        let (first_line, body) = message.split_once('\n').unwrap();
        format!("<tt><b>{first_line}</b>\n{body}</tt>")
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cfg::Error({})", self.message)
    }
}

impl<E: std::fmt::Display> From<E> for Error {
    fn from(e: E) -> Self {
        Self::new(format!("{e}"))
    }
}

/// Creates an error for value parsers, this does not specify the location of the
/// the error which is set by the main parser if parsing a value fails.
pub fn value_error(message: impl ToString, label: impl ToString) -> Error {
    Error::new(message).with_label(0..0, label)
}
