use libc::{signal, SIGCHLD, SIG_DFL, SIG_IGN};
use std::io::Result;
use std::process::{Command, ExitStatus, Stdio};

use crate::error::message_box;

pub fn ignore_sigchld (cfg: bool) {
  unsafe {
    signal (SIGCHLD, if cfg { SIG_IGN } else { SIG_DFL });
  }
}

struct Scoped_Default_SigChld;

impl Scoped_Default_SigChld {
  fn new () -> Self {
    ignore_sigchld (false);
    Self {}
  }
}

impl Drop for Scoped_Default_SigChld {
  fn drop (&mut self) {
    ignore_sigchld (true);
  }
}

/// Splits a commandline into it's elements, handling strings and escaped
/// characters. Strings are delimited by either `'` or `"`, any character after
/// a `\` is ignored.
pub fn split_commandline<S> (commandline: &str) -> Vec<S>
where
  S: std::str::FromStr,
{
  let mut result = Vec::new ();
  let mut elem = String::new ();
  let mut in_string: char = '\0';
  let mut ignore = false;
  for c in commandline.chars () {
    if ignore {
      ignore = false;
      continue;
    }
    match c {
      '\\' => {
        ignore = true;
      }
      '"' | '\'' => {
        if in_string == '\0' {
          in_string = c;
        } else if c == in_string {
          in_string = '\0';
        } else {
          elem.push (c);
        }
      }
      ' ' => {
        if in_string == '\0' {
          if !elem.is_empty () {
            result.push (unsafe { S::from_str (&elem).unwrap_unchecked () });
            elem = String::new ();
          }
        } else {
          elem.push (c);
        }
      }
      _ => {
        elem.push (c);
      }
    }
  }
  if !elem.is_empty () {
    result.push (unsafe { S::from_str (&elem).unwrap_unchecked () });
  }
  result
}

pub fn run (cmd: &[impl AsRef<str>]) -> Result<()> {
  Command::new (cmd[0].as_ref ())
    .args (cmd[1..].iter ().map (|a| a.as_ref ()))
    .spawn ()
    .map (|_| ())
}

pub fn run_or_message_box (cmd: &[impl AsRef<str>]) {
  if let Err (error) = run (cmd) {
    let commandline = cmd
      .iter ()
      .fold (String::new (), |a, b| a + " " + b.as_ref ());
    let body = format! ("{}\n{}", commandline, error);
    message_box ("Failed to run process:", &body);
  }
}

pub fn run_and_await (cmd: &[&str]) -> Result<ExitStatus> {
  let _guard = Scoped_Default_SigChld::new ();
  Command::new (cmd[0]).args (&cmd[1..]).status ()
}

pub fn run_and_await_with_output (cmd: &[&str]) -> Result<String> {
  let _guard = Scoped_Default_SigChld::new ();
  Command::new (cmd[0])
    .args (&cmd[1..])
    .stderr (Stdio::inherit ())
    .output ()
    .map (|output| String::from_utf8_lossy (&output.stdout).into_owned ())
}
