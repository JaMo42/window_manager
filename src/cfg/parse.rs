use super::{
    error::Error,
    scanner::{is_space, Scanner},
};
use crate::{error::fatal_error, x::Display};
use std::{fs::read_to_string, io};

pub enum SetError {
    InvalidKey(Error),
    InvalidValue(Error),
}

impl SetError {
    fn into_inner(self) -> Error {
        match self {
            Self::InvalidKey(e) | Self::InvalidValue(e) => e,
        }
    }
}

pub type SetResult<'a> = Result<(), SetError>;

/// Find the most similar item to `s` in `valid`.
/// If no item is sufficiently similar `None` is returned.
pub fn most_similar<'a, I>(s: &str, valid: I) -> Option<&'a str>
where
    I: Iterator<Item = &'a str>,
{
    let mut closest = "";
    let mut score = 0.0;
    for v in valid {
        let sim = strsim::jaro_winkler(v, s);
        if sim > score {
            closest = v;
            score = sim;
        }
    }
    if score >= 0.8 {
        Some(closest)
    } else {
        None
    }
}

pub trait Document {
    fn section(&mut self, path: &str) -> Result<&mut dyn Section, Error>;
}

pub trait Section {
    /// `section_name` is the name for the section defined in the `config` type
    /// to be used for error messages since the section type itself doesn't
    /// otherwise know its name in the config file.
    fn set(&mut self, section_name: &str, field: &str, scanner: &mut Scanner) -> SetResult;
}

pub trait Value: Sized {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error>;
}

/// Predicate matching section name characters.
fn is_section_char(c: char) -> bool {
    c.is_alphabetic() || c == '.' || c == '_' || c == '-'
}

/// Predicate matching value name characters.
fn is_value_char(c: char) -> bool {
    // note: this also matches a leading digit which can never be valid value
    // name as value names are always valid rust identifiers but this doesn't
    // really matter as it will just cause a parsing error.
    // In addition to this we allow `+` for the key bindings.
    c.is_ascii_alphanumeric() || c == '_' || c == '+'
}

pub struct Parser<'a> {
    display: &'a Display,
    path: &'a str,
    source: String,
}

impl<'a> Parser<'a> {
    pub fn new(display: &'a Display, path: &'a str) -> io::Result<Self> {
        Ok(Self {
            display,
            path,
            source: read_to_string(path)?,
        })
    }

    fn skip_non_content(scanner: &mut Scanner) {
        loop {
            match scanner.peek() {
                Some(space) if is_space(true)(space) => scanner.skip_space(true),
                Some(comment) if comment == '#' => {
                    scanner.rest_of_line();
                }
                _ => break,
            }
        }
    }

    /// Parses one line of content.
    fn parse_one_line(
        &mut self,
        scanner: &mut Scanner,
        section_path: &mut Option<String>,
        doc: &mut impl Document,
    ) -> Result<bool, Error> {
        Self::skip_non_content(scanner);
        if scanner.is_empty() {
            return Ok(false);
        }
        if scanner.starts_with("[") {
            scanner.next();
            scanner.skip_space(false);
            let section_name = scanner.some(is_section_char);
            scanner.skip_space(false);
            scanner.expect_eq(']')?;
            scanner.skip_space(true);
            let path = if section_name.starts_with('.') {
                let outer = section_path.as_deref().unwrap_or("");
                format!("{outer}{section_name}")
            } else {
                section_name.as_str().to_owned()
            };
            doc.section(path.as_str()).map_err(|mut e| {
                e.line = Some(section_name.location().line);
                e.label.as_mut().unwrap().0 = section_name.range();
                e
            })?;
            *section_path = Some(path);
            // TODO: store section
        } else {
            let key_location = *scanner.location();
            let key = scanner.some(is_value_char);
            if key.is_empty() {
                let loc = scanner.location();
                return Err(Error::new_with_line("missing field name", loc.line)
                    .with_label(loc.range(1), "epected field name"));
            }
            scanner.skip_space(false);
            scanner.expect_eq('=').map_err(|e| {
                e.with_why(key.range(), "because the previous token was a field name")
            })?;
            scanner.skip_space(false);
            let section_path = section_path.as_ref().ok_or_else(|| {
                key.as_error(
                    "assignment outside section",
                    "expected section before this assignment",
                )
            })?;
            // unwrap is safe here as we already checked its existence in the section header branch.
            let section = doc.section(section_path.as_str()).unwrap();
            let start = *scanner.location();
            let r = section.set(section_path.as_str(), key.as_str(), scanner);
            let end = *scanner.location();
            if let Err(set_error) = r {
                let real_span = match &set_error {
                    SetError::InvalidKey(_) => key.range(),
                    SetError::InvalidValue(_) => start.char_offset..end.char_offset,
                };
                let mut error = set_error.into_inner();
                if let Some((span, _)) = &mut error.label {
                    if *span == (0..0) {
                        *span = real_span;
                    }
                }
                error.line.get_or_insert(key_location.line);
                Err(error)?;
            }
            scanner.skip_space(false);
            scanner.expect_eq('\n')?;
        }
        Ok(true)
    }

    pub fn parse(&mut self, doc: &mut impl Document) {
        let mut section_path = None;
        let mut scanner = Scanner::new(unsafe { &*(&self.source as *const String) });
        loop {
            match self.parse_one_line(&mut scanner, &mut section_path, doc) {
                Ok(more) => {
                    if !more {
                        break;
                    }
                }
                Err(error) => {
                    fatal_error(self.display, error.to_markup(self.path, &self.source));
                }
            }
        }
    }
}
