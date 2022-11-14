use std::fs::{self, File};
use std::io::{self, BufRead};

#[derive(Default)]
pub struct Desktop_Entry {
  pub icon: Option<String>,
}

impl Desktop_Entry {
  fn find_file (application_name: &str) -> Option<String> {
    const BASE_PATH: &str = "/usr/share/applications";
    let path = format! ("{}/{}.desktop", BASE_PATH, application_name);
    // Some programs have names like `org.gnome.<application>`, for now we don't
    // bother trying to find those; I assume using the application name like
    // this just isn't the right approach.
    if fs::metadata (&path).is_ok () {
      Some (path)
    } else {
      None
    }
  }

  fn read_file (pathname: &str) -> io::Result<Desktop_Entry> {
    let mut result: Desktop_Entry = std::default::Default::default ();
    let file = File::open (pathname)?;
    let lines = io::BufReader::new (file).lines ();
    for line_or_error in lines {
      let line = line_or_error?;
      if line.starts_with ("Icon") {
        result.icon = Some (line.chars ().skip (5).collect ());
        break; // This is the only value we want for now
      }
    }
    Ok (result)
  }

  pub fn new (application_name: &str) -> Option<Desktop_Entry> {
    Self::find_file (application_name).and_then (|pathname| {
      match Self::read_file (&pathname) {
        Ok (desktop_entry) => Some (desktop_entry),
        Err (e) => {
          // Not really an error?
          log::error! ("Could not read {}: {}", pathname, e);
          None
        }
      }
    })
  }
}
