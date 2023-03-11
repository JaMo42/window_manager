use super::error::Error;
use std::{
    ops::{Deref, Range},
    str::CharIndices,
};

#[derive(Copy, Clone, Default, Debug)]
pub struct Location {
    pub line: usize,
    pub char_offset: usize,
    pub byte_offset: usize,
}

impl Location {
    pub fn range(&self, len: usize) -> Range<usize> {
        self.char_offset..(self.char_offset + len)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LocatedStr<'a> {
    s: &'a str,
    begin: Location,
    // The end is only needed for error reporting so we calculate it when needed
}

impl<'a> Deref for LocatedStr<'a> {
    type Target = str;

    fn deref(&self) -> &'a Self::Target {
        self.s
    }
}

impl<'a> LocatedStr<'a> {
    pub fn as_str(&self) -> &'a str {
        self.s
    }

    pub fn range(&self) -> Range<usize> {
        self.begin.byte_offset..(self.begin.byte_offset + self.s.len())
    }

    pub fn location(&self) -> &Location {
        &self.begin
    }

    /// Creates an error on the line of the string with an error label using
    /// the span of the string.
    pub fn as_error(&self, message: impl ToString, label: impl ToString) -> Error {
        Error::new_with_line(message, self.begin.line).with_label(self.range(), label)
    }
}

impl std::fmt::Display for LocatedStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.s)
    }
}

#[derive(Debug, Clone)]
pub struct Scanner<'a> {
    source: &'a str,
    it: CharIndices<'a>,
    peeked: Option<(Option<char>, Location)>,
    location: Location,
}

impl<'a> Scanner<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            source: s,
            it: s.char_indices(),
            peeked: None,
            location: Location::default(),
        }
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn as_str(&mut self) -> &str {
        &self.source[self.peek_location().byte_offset..]
    }

    pub fn starts_with(&mut self, s: &str) -> bool {
        self.as_str().starts_with(s)
    }

    pub fn peek(&mut self) -> Option<char> {
        if let Some(peeked) = self.peeked {
            peeked.0
        } else {
            let current_location = self.location;
            let maybe_next = self.next();
            self.peeked = Some((maybe_next, self.location));
            self.location = current_location;
            maybe_next
        }
    }

    pub fn next_if(&mut self, predicate: impl FnOnce(char) -> bool) -> Option<char> {
        let current_location = self.location;
        match self.next() {
            Some(next) if predicate(next) => Some(next),
            otherwise => {
                self.peeked = Some((otherwise, self.location));
                self.location = current_location;
                None
            }
        }
    }

    pub fn is_empty(&mut self) -> bool {
        self.peek().is_none()
    }

    pub fn expect(
        &mut self,
        what: impl ToString,
        predicate: impl FnOnce(char) -> bool,
    ) -> Result<char, Error> {
        let loc = self.location;
        match self.next() {
            Some(next) if predicate(next) => Ok(next),
            otherwise => Err(char_mismatch_error(what, otherwise, &loc)),
        }
    }

    pub fn expect_eq(&mut self, expected: char) -> Result<char, Error> {
        self.expect(display_char(expected), |c| c == expected)
    }

    pub fn peek_location(&mut self) -> &Location {
        self.peek();
        match &self.peeked {
            Some((_, location)) => location,
            _ => &self.location,
        }
    }

    pub fn some(&mut self, mut predicate: impl FnMut(char) -> bool) -> LocatedStr<'a> {
        let start = *self.peek_location();
        while self.next_if(&mut predicate).is_some() {}
        let mut end = *self.peek_location();
        if self.peek().is_none() {
            end.byte_offset += 1;
        }
        LocatedStr {
            s: &self.source[start.byte_offset..end.byte_offset],
            begin: start,
        }
    }

    pub fn digits(&mut self) -> LocatedStr<'a> {
        self.some(|c| c.is_ascii_digit())
    }

    pub fn skip_space(&mut self, newline: bool) {
        let p = is_space(newline);
        while self.next_if(p).is_some() {}
    }

    pub fn rest_of_line(&mut self) -> LocatedStr<'a> {
        self.some(|c| c != '\n')
    }

    /// Implementation for [`all_until_except`] if `until` is `None`.
    fn all_except(&mut self, except: Option<char>) -> Result<LocatedStr<'a>, Error> {
        if except.is_none() {
            return Ok(LocatedStr {
                s: &self.source[self.location.byte_offset..],
                begin: self.location,
            });
        }
        let except = except.unwrap();
        let rest = self.some(|c| c != except);
        if !self.is_empty() {
            Err(
                Error::new_with_line("unexpected character", self.location.line).with_label(
                    self.location.range(1),
                    format!("expected {} before end of input", display_char(except)),
                ),
            )
        } else {
            Ok(rest)
        }
    }

    pub fn all_until_except(
        &mut self,
        until: Option<char>,
        except: Option<char>,
    ) -> Result<LocatedStr<'a>, Error> {
        if until.is_none() {
            return self.all_except(except);
        }
        let until = until.unwrap();
        let check_except = except.unwrap_or(unsafe { char::from_u32_unchecked(u32::MAX) });
        let matched = self.some(|c| c != until && c != check_except);
        match self.peek() {
            Some(m) if m == until => Ok(matched),
            otherwise => {
                let what = if let Some(c) = otherwise {
                    display_char(c)
                } else {
                    "end of input".to_string()
                };
                Err(
                    Error::new_with_line("unexpected character", self.location.line).with_label(
                        self.location.range(1),
                        format!("expected {} before {}", display_char(until), what),
                    ),
                )
            }
        }
    }

    #[allow(dead_code)]
    pub fn parse(&mut self, fmt: &str, ignore_whitespace: bool) -> Result<Vec<LocatedStr>, Error> {
        let mut strings = Vec::with_capacity(4);
        let mut fmt = Scanner::new(fmt);
        if ignore_whitespace {
            fmt.skip_space(false);
        }
        while let Some(fmt_char) = fmt.next() {
            if fmt_char == '{' {
                if let Some(after) = fmt.next() {
                    if after == '}' {
                        strings.push(self.all_until_except(fmt.peek(), None)?);
                        continue;
                    } else if after.is_ascii_digit() {
                        let count = fmt.some(|c| c.is_ascii_digit()).as_str();
                        let count = format!("{after}{count}");
                        // Add 1 here so we can increment `have` before the check in the predicate
                        let count = count.parse::<usize>().unwrap() + 1;
                        let mut have = 0;
                        strings.push(self.some(|_| {
                            have += 1;
                            count != have
                        }));
                        if fmt.next() != Some('}') {
                            panic!("invalid format string: expected `}}` after count");
                        }
                        continue;
                    }
                }
                panic!("invalid format string: expected count or `}}` after `{{`");
            }
            let my_next = self.peek();
            if my_next != Some(fmt_char) {
                return Err(char_mismatch_error(
                    display_char(fmt_char),
                    my_next,
                    &self.location,
                ));
            }
            self.next();
        }
        Ok(strings)
    }
}

impl Iterator for Scanner<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(peeked) = self.peeked.take() {
            self.location = peeked.1;
            peeked.0
        } else if let Some((byte_offset, next)) = self.it.next() {
            if next == '\n' {
                self.location.line += 1;
            }
            self.location.char_offset += 1;
            self.location.byte_offset = byte_offset;
            Some(next)
        } else {
            None
        }
    }
}

fn display_char(c: char) -> String {
    if c == '\n' {
        "newline".to_string()
    } else {
        format!("`{}`", c)
    }
}

fn char_mismatch_error(expected: impl ToString, actual: Option<char>, loc: &Location) -> Error {
    let expected = expected.to_string();
    let label = format!("expected {expected}");
    let message = match actual {
        Some(actual) => {
            let actual = display_char(actual);
            format!("expected {expected}, found {actual}")
        }
        None => "unexpected end of input".to_string(),
    };
    Error::new_with_line(message, loc.line).with_label(loc.range(1), label)
}

/// Returns a predicate matching whitespace.
pub fn is_space(newline: bool) -> fn(char) -> bool {
    if newline {
        |c: char| c == ' ' || c == '\t' || c == '\n'
    } else {
        |c: char| c == ' ' || c == '\t'
    }
}
