use crate::cfg::error::{value_error, Error};
use crate::cfg::parse::{most_similar, Document, Section, SetError, SetResult, Value};
use crate::cfg::scanner::Scanner;
use crate::color::Color;
use crate::color_scheme::{ColorConfig, ColorScheme, ColorSchemeConfig};
use crate::config::{Action, KeyBindings};
use crate::draw::Alignment;
use crate::process::split_commandline;
use crate::{action, platform, x, AnyResult};
use crate::{config::Size, monitors::WindowAreaPadding};
use pango::FontDescription;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;

const fn is_ascii_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Gets the value of a single valid hex digit
fn hex_digit_value(c: char) -> u8 {
    let as_byte = c as u8;
    const RANGES: [(u8, RangeInclusive<u8>); 3] =
        [(0, b'0'..=b'9'), (10, b'a'..=b'f'), (10, b'A'..=b'F')];
    for r in RANGES {
        if r.1.contains(&as_byte) {
            return r.0 + as_byte - r.1.start();
        }
    }
    unreachable!()
}

/// Gets the byte value of 2 valid hex digits
fn hex_byte_value(chars: &[char]) -> u8 {
    (hex_digit_value(chars[0]) << 4) | hex_digit_value(chars[1])
}

/// Parses a list of values using the given delimiter and whitespace rules.
fn parse_list<T: Value>(
    scanner: &mut Scanner,
    open: char,
    close: char,
    delim: char,
    allow_newlines: bool,
) -> Result<Vec<T>, Error> {
    let mut v = Vec::new();
    let after_value_what = format!("`{}` or `{}`", delim, close);
    scanner.expect_eq(open)?;
    scanner.skip_space(allow_newlines);
    if scanner.peek() == Some(close) {
        return Ok(v);
    }
    loop {
        scanner.skip_space(allow_newlines);
        v.push(T::parse(scanner)?);
        scanner.skip_space(allow_newlines);
        if scanner.expect(&after_value_what, |c| c == delim || c == close)? == close {
            break;
        }
    }
    Ok(v)
}

////////////////////////////////////////////////////////////////////////////////
// Keybindings section
////////////////////////////////////////////////////////////////////////////////

fn verify_mod(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "win" | "shift" | "alt" | "ctrl" | "mod"
    )
}

enum VerifyModsAndKeyResult<'a> {
    Ok,
    InvalidKey(&'a str),
    InvalidMod(&'a str),
}

fn verify_mods_and_key(s: &str) -> VerifyModsAndKeyResult {
    let mut it = s.split('+').peekable();
    while let Some(i) = it.next() {
        if it.peek().is_some() {
            if !verify_mod(i) {
                return VerifyModsAndKeyResult::InvalidMod(i);
            }
        } else if x::string_to_keysym(i).is_none() {
            return VerifyModsAndKeyResult::InvalidKey(i);
        }
    }
    VerifyModsAndKeyResult::Ok
}

impl Section for KeyBindings {
    fn set(&mut self, _: &str, mods_and_key: &str, scanner: &mut Scanner) -> SetResult {
        match verify_mods_and_key(mods_and_key) {
            VerifyModsAndKeyResult::InvalidKey(key) => {
                return Err(SetError::InvalidKey(value_error(
                    "invalid key",
                    format!("`{}` does not name a known key", key),
                )));
            }
            VerifyModsAndKeyResult::InvalidMod(modifier) => {
                return Err(SetError::InvalidKey(
          value_error(
            "invalid modifier",
            format!("`{}` does not name a known modifier", modifier),
          )
          .with_help("valid modifier names are `Shift`, `Ctrl`, `Mod`, `Win`, `Super`, and `Alt`"),
        ));
            }
            _ => {}
        };
        let action = Action::parse(scanner).map_err(SetError::InvalidValue)?;
        self.push(mods_and_key.to_string(), action);
        Ok(())
    }
}

impl Value for Action {
    #[rustfmt::skip]
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        use Action::*;
        static TABLE: Mutex<Option<HashMap<&'static str, Action>>> = Mutex::new(None);
        let mut lock = TABLE.lock();
        if lock.is_none() {
            *lock = Some(HashMap::from([
                ("quit", Generic(action::quit)),
                ("quit_dialog", Generic(action::quit_dialog_action)),
                ("snap_left", Client(action::snap_left)),
                ("snap_right", Client(action::snap_right)),
                ("snap_up", Client(action::snap_up)),
                ("snap_down", Client(action::snap_down)),
                ("maximize", Client(action::maximize)),
                ("unsnap_or_center", Client(action::unsnap_or_center)),
                ("close_window", Client(action::close_client)),
                ("raise_all", Generic(action::raise_all)),
                ("decrease_volume", Generic(platform::actions::decrease_volume)),
                ("increase_volume", Generic(platform::actions::increase_volume)),
                ("mute_volume", Generic(platform::actions::mute_volume)),
                ("move_to_next_monitor", Client(action::move_to_next_monitor)),
                ("move_to_prev_monitor", Client(action::move_to_prev_monitor)),
            ]));
        }
        let table = lock.as_ref().unwrap();
        let name = scanner.rest_of_line();
        if name.starts_with('$') {
            let mut chars = name.chars();
            chars.next().unwrap();
            let cmd = chars.as_str();
            Ok(Action::Launch(split_commandline(cmd)))
        } else if let Some(action) = table.get(name.to_lowercase().as_str()) {
            Ok(action.clone())
        } else {
            let mut error =
                Error::new_with_line(format!("no such action: {name}"), name.location().line);
            if let Some(similar) = most_similar(name.as_str(), table.keys().cloned()) {
                error = error.with_label(
                    name.range(),
                    format!("help: an action with a similar name exists: {similar}"),
                );
            }
            Err(error)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Value types
////////////////////////////////////////////////////////////////////////////////

trait IsUint {}
impl IsUint for u8 {}
impl IsUint for u16 {}
impl IsUint for u32 {}
impl IsUint for u64 {}
impl IsUint for usize {}

impl<T> Value for T
where
    T: IsUint + std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let digits = scanner.digits();
        digits
            .parse::<T>()
            .map_err(|e| value_error("invalid number value", format!("{e}")))
    }
}

impl Value for f64 {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let dec = scanner
            .digits()
            .parse::<u16>()
            // This is the only operation that could fail due to being empty.
            .map_err(|e| value_error("invalid number value", format!("{e}")))?;
        let mut flt = 0u16;
        if scanner.next_if(|c| c == '.').is_some()
            && scanner.peek().map(is_ascii_digit).unwrap_or(false)
        {
            flt = scanner.digits().parse().unwrap();
        }
        let mut r = flt as f64;
        while r >= 1.0 {
            r /= 10.0;
        }
        r += dec as f64;
        Ok(r)
    }
}

impl Value for bool {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let next_word = scanner.some(|c| c.is_ascii_alphabetic());
        match next_word.as_str().to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "yes" => Ok(true),
            "false" => Ok(false),
            "no" => Ok(false),
            _ => Err(next_word.as_error(
                "invalid boolean value",
                "expected `true`, `yes`, `false`, or `no`",
            )),
        }
    }
}

impl Value for String {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let delim = scanner.next().unwrap();
        if delim != '"' && delim != '\'' {
            return Err(value_error("invalid string value", "expected `\"` or `'`"));
        }
        let s = scanner.all_until_except(Some(delim), Some('\n'))?;
        scanner.next();
        Ok(s.to_string())
    }
}

impl Value for FontDescription {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let s = String::parse(scanner)?;
        Ok(FontDescription::from_string(&s))
    }
}

impl Value for Size {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        const SUFFIXES: [&str; 6] = ["", "px", "mm", "cm", "%", "em"];
        let num = f64::parse(scanner)?;
        let suffix = Some(
            scanner
                .some(|c| c.is_ascii_alphabetic() || c == '%')
                .as_str(),
        )
        .filter(|s| SUFFIXES.contains(s))
        .ok_or_else(|| value_error("invalid size value", "invalid suffix"))?;
        Ok(Self::new_with_suffix(num, suffix))
    }
}

impl Value for WindowAreaPadding {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let sizes = <(Size, Size, Size, Size)>::parse(scanner)?;
        let this = Self {
            top: sizes.0,
            bottom: sizes.1,
            left: sizes.2,
            right: sizes.3,
        };
        if !this.is_valid() {
            Err(value_error(
                "invalid window area padding",
                "monitor padding cannot be relative to a font size",
            ))
        } else {
            Ok(this)
        }
    }
}

impl Value for Alignment {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        // We currently only use this for horizontal alignment so we can just
        // parse those value and not worry about validating them.
        let next_word = scanner.some(|c| c.is_alphabetic());
        match next_word.as_str().to_ascii_lowercase().as_str() {
            "left" => Ok(Alignment::LEFT),
            "center" => Ok(Alignment::CENTER),
            "right" => Ok(Alignment::RIGHT),
            _ => Err(next_word.as_error(
                "invalid alignment value",
                "expected `Left`, `Center`, or `Right`",
            )),
        }
    }
}

impl Value for Color {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        if scanner.starts_with("#") {
            // Using `Scanner::parse` here is impractical since we don't know
            // if the color will contain an alpha value.
            scanner.next();
            let hex_chars = scanner.some(|c| c.is_ascii_hexdigit());
            if hex_chars.len() != 6 && hex_chars.len() != 8 {
                return Err(value_error(
                    "invalid hex color",
                    "expected `#RRGGBB` or `#RRGGBBAA`",
                ));
            }
            let components: Vec<_> = hex_chars.chars().collect();
            let components: Vec<_> = components.chunks(2).map(hex_byte_value).collect();
            Ok(Color::new(
                components[0] as f64 / 255.0,
                components[1] as f64 / 255.0,
                components[2] as f64 / 255.0,
                if components.len() == 4 {
                    components[3] as f64 / 255.0
                } else {
                    1.0
                },
            ))
        } else if scanner.starts_with("rgb(") {
            scanner.skip(3).for_each(drop);
            let components: Vec<u8> = parse_list(scanner, '(', ')', ',', false)?;
            Ok(Color::new_rgb(
                components[0] as f64 / 255.0,
                components[1] as f64 / 255.0,
                components[2] as f64 / 255.0,
            ))
        } else if scanner.starts_with("rgba(") {
            scanner.skip(4).for_each(drop);
            let components: Vec<u8> = parse_list(scanner, '(', ')', ',', false)?;
            Ok(Color::new(
                components[0] as f64 / 255.0,
                components[1] as f64 / 255.0,
                components[2] as f64 / 255.0,
                components[3] as f64 / 255.0,
            ))
        } else {
            Err(
        value_error("invalid color value", "expected color").with_help(
          "valid formats are `#RRGGBB`, `#RRGGBBAA`, `rgb(r, g, b)`, and `rgba(r, g, b, a)`",
        ),
      )
        }
    }
}

impl<T> Value for Vec<T>
where
    T: Value,
{
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        parse_list(scanner, '[', ']', ',', true)
    }
}

impl<T> Value for (T, T)
where
    T: Value,
{
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let elems = <Vec<T>>::parse(scanner)?;
        if elems.len() != 2 {
            return Err(value_error("invalid element count", "expected 2 values"));
        }
        let mut elems = elems.into_iter();
        Ok((elems.next().unwrap(), elems.next().unwrap()))
    }
}

impl<T> Value for (T, T, T, T)
where
    T: Value,
{
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let elems = <Vec<T>>::parse(scanner)?;
        if elems.len() != 4 {
            return Err(value_error("invalid element count", "expected 4 values"));
        }
        let mut elems = elems.into_iter();
        Ok((
            elems.next().unwrap(),
            elems.next().unwrap(),
            elems.next().unwrap(),
            elems.next().unwrap(),
        ))
    }
}
/*
pub trait StringValidator {
    fn validate(s: &str) -> Result<(), Error>;
}

pub struct ValidatedString<V: StringValidator> {
    inner: String,
    v_marker: PhantomData<V>,
}

impl<V: StringValidator> Deref for ValidatedString<V> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<V: StringValidator> Value for ValidatedString<V> {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        let start = *scanner.location();
        let s = String::parse(scanner)?;
        let end = *scanner.location();
        V::validate(&s)?;
        Ok(Self {
            inner: s,
            v_marker: PhantomData,
        })
    }
}

pub struct WindowButtonNameValidator;
impl StringValidator for WindowButtonNameValidator {
    fn validate(s: &str) -> Result<(), Error> {
        const VALID: [&str; 3] = ["minimize", "maximize", "close"];
        if VALID.contains(&s) {
            Ok(())
        } else {
            let mut error = Error::new("invalid button name");
            if let Some(similar) = most_similar(s, VALID.into_iter()) {
                error = error.with_label(0..0, format!("help: a similar name exists: `{similar}`"));
            } else {
                error = error
                    .with_label(0..0, "expected button name")
                    .with_help("valid button names are `close`, `maximize`, and `minimize`");
            }
            Err(error)
        }
    }
}
*/

////////////////////////////////////////////////////////////////////////////////
// Color scheme
////////////////////////////////////////////////////////////////////////////////

// TODO: completely restructure this to use the `parsed_config` macro.

#[derive(Default)]
pub struct Palette {
    defs: BTreeMap<String, Color>,
}

impl Section for Palette {
    fn set(&mut self, _: &str, field: &str, scanner: &mut Scanner) -> SetResult {
        self.defs.insert(
            field.to_owned(),
            Color::parse(scanner).map_err(SetError::InvalidValue)?,
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct ColorSchemeParser {
    config: ColorSchemeConfig,
    palette: Palette,
}

impl ColorSchemeParser {
    pub fn finish(self) -> AnyResult<ColorScheme> {
        ColorScheme::new(&self.config, &self.palette.defs)
    }
}

impl Document for ColorSchemeParser {
    fn section(&mut self, path: &str) -> Result<&mut dyn Section, Error> {
        if &path.to_ascii_lowercase() == "palette" {
            Ok(&mut self.palette)
        } else {
            // Could already check the section name here as a color name must start
            // with it but we can't really check for similar names so we just skip
            // the check here.
            Ok(self)
        }
    }
}

impl Section for ColorSchemeParser {
    fn set(&mut self, section_name: &str, field: &str, scanner: &mut Scanner) -> SetResult {
        let elem = format!("{}.{}", section_name, field);
        let cfg = ColorConfig::parse(scanner).map_err(SetError::InvalidValue)?;
        self.config
            .set(&elem, cfg)
            .map_err(|e| SetError::InvalidKey(Error::new(e)))?;
        Ok(())
    }
}

impl Value for ColorConfig {
    fn parse(scanner: &mut Scanner) -> Result<Self, Error> {
        if scanner.starts_with("#") || scanner.starts_with("rgb(") || scanner.starts_with("rgba(") {
            let color = Color::parse(scanner)?;
            Ok(ColorConfig::Value(color))
        } else {
            let link_name = scanner.some(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            // Can't verify link name here since we don't know what the palette
            // colors are.
            Ok(ColorConfig::Link(link_name.as_str().to_owned()))
        }
    }
}
