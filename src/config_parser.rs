use std::iter::Peekable;
use std::path::Path;
use std::io::{self, BufRead};
use std::fs::File;
use std::os::raw::{c_int, c_uint};
use super::core::*;
use super::config::Height;

fn read_lines<P: AsRef<Path>> (path: P) -> io::Result<io::Lines<io::BufReader<File>>> {
  let f = File::open (path)?;
  Ok (io::BufReader::new (f).lines ())
}

fn append_lines (
  str: &mut String, file_content: io::Lines<io::BufReader<File>>, directory: &Path
) -> io::Result<()> {
  for maybe_line in file_content {
    match maybe_line {
      Ok (line) => {
        if line.starts_with ("%include ") {
          let filename: String = line.chars ().skip (9).collect ();
          let pathname = directory.join (filename);
          let content = read_lines (pathname)?;
          append_lines (str, content, directory)?;
        } else {
          str.push_str (line.as_str ());
          str.push ('\n');
        }
      }
      Err (e) => {
        return Err (e);
      }
    }
  }
  Ok (())
}

/// Reads the config file and expands includes
pub fn read_file (path: &String) -> io::Result<String> {
  let mut s = String::new ();
  let directory = Path::new (&path).parent ().unwrap ();
  let f = read_lines (path)?;
  append_lines (&mut s, f, directory)?;
  Ok (s)
}

pub enum Definition_Type {
  Workspaces (usize),
  Gaps (c_uint),
  Padding (c_int, c_int, c_int, c_int),
  Border (c_uint),
  Meta (String),
  Mod (c_uint),
  Bind_Key (c_uint, String, String),
  Bind_Command (c_uint, String, String),
  Color (String, String),
  Def_Color (String, String),
  Bar_Font (String),
  Bar_Opacity (u8),
  Bar_Time_Format (String),
  Bar_Power_Supply (String),
  Bar_Height (Height),
  Title_Font (String),
  Title_Height (Height),
  Title_Position (String),
  Left_Buttons (Vec<String>),
  Right_Buttons (Vec<String>),
  Button_Icon_Size (u8),
  Circle_Buttons,
  Default_Notification_Timeout (i32)
}

pub struct Parser<Chars: Iterator<Item=char>> {
  chars: Peekable<Chars>,
  line: usize,
  column: usize,
  exhausted: bool,
  user_mod: c_uint,
  thing: String,
  thing_col: usize
}

impl<Chars: Iterator<Item=char>> Parser<Chars> {
  pub fn new (chars: Chars) -> Self {
    Self {
      chars: chars.peekable (),
      line: 1,
      column: 0,
      exhausted: false,
      user_mod: MOD_WIN,
      thing: String::new (),
      thing_col: 0
    }
  }

  fn drop_line (&mut self) {
    while self.chars.next_if(|x| *x != '\n').is_some() {
    }
    self.chars.next ();
    self.line += 1;
    self.column = 0;
    if self.chars.peek ().is_none () {
      self.exhausted = true;
    }
  }

  fn trim_whitespace (&mut self) {
    while self.chars.next_if (|x| x.is_whitespace () && *x != '\n').is_some () {
      self.column += 1;
    }
  }

  fn next_thing (&mut self) -> String {
    self.trim_whitespace ();
    self.thing_col = self.column;
    let mut thing = String::new ();
    while let Some (c) = self.chars.next_if (|x| !x.is_whitespace ()) {
      thing.push (c);
      self.column += 1;
    }
    self.thing = thing.clone ();
    thing
  }

  fn rest_of_line (&mut self) -> String {
    self.trim_whitespace ();
    self.thing_col = self.column;
    let mut line = String::new ();
    while let Some (c) = self.chars.next_if (|x| *x != '\n') {
      line.push (c);
      self.column += 1;
    }
    self.thing = line.clone ();
    line
  }

  fn parse_number<T: std::str::FromStr> (&mut self) -> T {
    if let Ok (n) = self.next_thing ().parse::<T> () {
      n
    }
    else {
      self.fail ("Expected a number");
    }
  }

  fn skip_blank_and_comments (&mut self) {
    while let Some (c) = self.chars.next_if (|x| *x == '#' || *x == '\n') {
      if c == '#' {
        self.drop_line ();
      }
      else {
        self.line += 1;
      }
    }
  }

  fn parse_height (&mut self) -> Height {
    let thing = self.next_thing ();
    let is_plus = thing.starts_with ('+');
    let num_str = if is_plus {
      let mut it = thing.chars ();
      it.next ();
      it.as_str ()
    }
    else {
      thing.as_str ()
    };
    if let Ok (n) = num_str.parse::<u32> () {
      if is_plus || n == 0 {
        Height::FontPlus (n)
      }
      else {
        Height::Absolute (n)
      }
    }
    else {
      self.fail ("Expected a number");
    }
  }

  fn parse_choice (&mut self, choices: &[&str]) -> String {
    let thing = self.next_thing ();
    if choices.contains (&thing.as_str ()) {
      thing
    } else {
      self.fail (&format! ("Expected one of: {}", choices.join (", ")));
    }
  }

  fn parse_percentage (&mut self) -> u8 {
    let mut thing = self.next_thing ();
    if !thing.ends_with ('%') {
      self.fail ("Expected percentage");
    }
    thing.pop ();
    if let Ok (n) = thing.parse::<u8> () {
      if n > 100 {
        self.fail ("Percentage should be in range 0~100")
      }
      n
    } else {
      self.fail ("Expected percentage");
    }
  }

  fn parse_line (&mut self) -> Definition_Type {
    use Definition_Type::*;

    self.skip_blank_and_comments ();
    if self.chars.peek ().is_none () {
      self.exhausted = true;
      // Just return anything since the iterator return None
      return Gaps (0);
    }

    match self.next_thing ().as_str () {
      "workspaces" => Workspaces (self.parse_number ()),
      "gaps" => Gaps (self.parse_number ()),
      "pad" => Padding (
        self.parse_number (),
        self.parse_number (),
        self.parse_number (),
        self.parse_number ()
      ),
      "border" => Border (self.parse_number ()),
      "meta" => Meta (self.next_thing ()),
      "mod" => {
        let mods = modifiers_from_string (self.next_thing ());
        self.user_mod = mods;
        Mod (mods)
      }
      "bind" => {
        let (mods, key) = mods_and_key_from_string (self.next_thing (), self.user_mod);
        let next_thing = self.next_thing ();
        if next_thing == "$" {
          Bind_Command (mods, key, self.rest_of_line ())
        }
        else {
          Bind_Key (mods, key, next_thing)
        }
      }
      "color" => Color (self.next_thing (), self.next_thing ()),
      "def_color" => Def_Color (self.next_thing (), self.next_thing ()),
      "bar_font" => Bar_Font (self.rest_of_line ().trim ().to_string ()),
      "bar_opacity" => Bar_Opacity (self.parse_percentage ()),
      "bar_time_format" => Bar_Time_Format (self.rest_of_line ()),
      "bar_power_supply" => Bar_Power_Supply (self.next_thing ()),
      "bar_height" => Bar_Height (self.parse_height ()),
      "window_title_font" => Title_Font (self.rest_of_line ().trim ().to_string ()),
      "window_title_height" => Title_Height (self.parse_height ()),
      "window_title_position" => Title_Position (self.parse_choice (&["left", "center", "right"])),
      "left_buttons" => Left_Buttons (
        self.rest_of_line ()
          .split (' ')
          .map (|s| s.to_string ())
          .collect ()
      ),
      "right_buttons" => Right_Buttons (
        self.rest_of_line ()
          .split (' ')
          .map (|s| s.to_string ())
          .collect ()
      ),
      "button_icon_size" => Button_Icon_Size (self.parse_percentage ()),
      "circle_buttons" => Circle_Buttons,
      "default_notification_timeout" => Default_Notification_Timeout (self.parse_number ()),
      _ => {
        self.fail ("Unknown keyword");
      }
    }
  }

  fn fail (&mut self, description: &str) -> ! {
    eprintln! ("config:{}:{} at {}: {}", self.line, self.thing_col, self.thing, description);
    log::error! ("config:{}:{} at {}: {}", self.line, self.thing_col, self.thing, description);
    std::process::exit (1);
  }
}


impl<Chars: Iterator<Item=char>> Iterator for Parser<Chars> {
  type Item = Definition_Type;

  fn next (&mut self) -> Option<Self::Item> {
    let def = self.parse_line ();
    if self.exhausted {
      None
    }
    else {
      self.drop_line ();
      Some (def)
    }
  }
}


fn str2mod (s: &str, m: c_uint) -> c_uint {
  match s {
    "Win" => MOD_WIN,
    "Shift" => MOD_SHIFT,
    "Alt" => MOD_ALT,
    "Ctrl" => MOD_CTRL,
    "Mod" => m,
    _ => 0
  }
}


fn modifiers_from_string (s: String) -> c_uint {
  let mut mods = 0;
  for mod_str in s.split ('+') {
    mods |= str2mod (mod_str, 0);
  }
  mods
}

fn mods_and_key_from_string (s: String, user_mod: c_uint) -> (c_uint, String) {
  let mut mods = 0;
  let mut key = String::new ();
  let mut it = s.split ('+').peekable ();
  while let Some (i) = it.next () {
    if it.peek ().is_some () {
      mods |= str2mod (i, user_mod);
    }
    else {
      key = i.to_string ();
    }
  }
  (mods, key)
}
