use freedesktop_entry_parser::parse_entry;
use std::fs;
use std::io;

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
      // TODO: Should be the translated name.
      .replace ("%c", name)
      .replace ("%k", pathname)
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
      name: get! ("Name").unwrap (), // Name is a required field
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
            name: name.to_owned (),
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
