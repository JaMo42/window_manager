use freedesktop_entry_parser::{parse_entry, AttrSelector};
use std::fs;
use std::io;

fn get_locale () -> Option<(String, Option<String>, Option<String>)> {
  let mut locale = std::env::var ("LC_MESSAGES")
    .or_else (|_| std::env::var ("LANG"))
    .ok ()?;
  let mut country = None;
  let mut modifier = None;
  if let Some (modifier_tag) = locale.chars ().position (|c| c == '@') {
    modifier = Some (locale[(modifier_tag + 1)..].to_string ());
    locale.replace_range (modifier_tag.., "");
  }
  if let Some (encoding) = locale.chars ().position (|c| c == '.') {
    locale.replace_range (encoding.., "");
  }
  if let Some (country_tag) = locale.chars ().position (|c| c == '_') {
    country = Some (locale[(country_tag + 1)..].to_string ());
    locale.replace_range (country_tag.., "");
  }
  Some ((locale, country, modifier))
}

// https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#extra-actions
#[derive(Default)]
pub struct Desktop_Action {
  pub name: String,
  pub exec: Option<String>,
  pub icon: Option<String>,
}

// https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#recognized-keys
#[derive(Default)]
pub struct Desktop_Entry {
  pub name: String,
  pub icon: Option<String>,
  pub exec: Option<String>,
  pub actions: Vec<Desktop_Action>,
}

impl Desktop_Entry {
  fn find_file (application_name: &str) -> Option<String> {
    const BASE_PATH: &str = "/usr/share/applications";
    let path = format! ("{}/{}.desktop", BASE_PATH, application_name);
    if fs::metadata (&path).is_ok () {
      Some (path)
    } else {
      // Search for file ending in `<app-name>.desktop`, if more than one
      // matches return none
      let mut found = None;
      let look_for = format! ("{}.desktop", application_name);
      for entry in fs::read_dir ("/usr/share/applications")
        .unwrap ()
        .flatten ()
      {
        if entry.file_name ().to_str ()?.ends_with (&look_for) {
          if found.is_some () {
            // Ambiguous match
            return None;
          } else {
            found = Some (entry.path ().to_str ()?.to_owned ());
          }
        }
      }
      found
    }
  }

  fn expand_exec (exec: String, icon: Option<&String>, name: &str, pathname: &str) -> String {
    exec
      .replace ("%F", "")
      .replace ("%f", "")
      .replace ("%U", "")
      .replace ("%u", "")
      .replace (
        "%i",
        // Value of the icon key or nothing
        &icon.map (|i| format! ("--icon {}", i)).unwrap_or_default (),
      )
      .replace ("%c", name)
      .replace ("%k", pathname)
  }

  // https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html#localized-keys
  fn get_localized_name<S: AsRef<str>> (section: &AttrSelector<'_, S>) -> Option<String> {
    macro_rules! check {
      ($param:expr) => {
        if let Some (name) = section.attr_with_param ("Name", $param) {
          return Some (name.to_owned ());
        }
      };
    }
    let (lang, country, modifier) = get_locale ()?;
    if let Some (country) = &country {
      if let Some (modifier) = &modifier {
        check! (format! ("{}_{}@{}", lang, country, modifier));
      }
      check! (format! ("{}_{}", lang, country));
    }
    if let Some (modifier) = &modifier {
      check! (format! ("{}@{}", lang, modifier));
    }
    check! (lang);
    None
  }

  fn read_file (pathname: &str) -> io::Result<Desktop_Entry> {
    let entry = parse_entry (pathname)?;
    let de = entry.section ("Desktop Entry");
    macro_rules! get {
      ($attr:expr) => {
        de.attr ($attr).map (|s| s.to_owned ())
      };
    }
    let mut result = Desktop_Entry {
      name: Self::get_localized_name (&de).unwrap_or_else (|| get! ("Name").unwrap ()),
      icon: get! ("Icon"),
      exec: get! ("Exec"),
      actions: Vec::new (),
    };
    // Expand Exec field codes, this is done specifically for the dock right now
    result.exec = result
      .exec
      .map (|s| Self::expand_exec (s, result.icon.as_ref (), &result.name, pathname));
    if let Some (actions) = get! ("Actions") {
      for action in actions.split (';') {
        if action.is_empty () {
          // We get an extra empty element since the actions strings always
          // terminated by a samicolon
          break;
        }
        let section_name = format! ("Desktop Action {}", action);
        let section = entry.section (section_name);
        if let Some (name) = section.attr ("Name") {
          let mut action = Desktop_Action {
            name: Self::get_localized_name (&section).unwrap_or_else (|| name.to_owned ()),
            exec: section.attr ("Exec").map (|s| s.to_owned ()),
            icon: section.attr ("Icon").map (|s| s.to_owned ()),
          };
          action.exec = action
            .exec
            .map (|s| Self::expand_exec (s, result.icon.as_ref (), &result.name, pathname));
          result.actions.push (action);
        }
      }
    }
    Ok (result)
  }

  pub fn new (application_name: &str) -> Option<Desktop_Entry> {
    Self::find_file (application_name).and_then (|pathname| match Self::read_file (&pathname) {
      Ok (desktop_entry) => Some (desktop_entry),
      Err (e) => {
        log::error! ("Could not read {}: {}", pathname, e);
        None
      }
    })
  }

  pub fn entry_name (application_name: &str) -> Option<String> {
    Self::find_file (application_name).and_then (|pathname| {
      Some (
        std::path::Path::new (&pathname)
          .file_stem ()?
          .to_str ()?
          .to_owned (),
      )
    })
  }
}
