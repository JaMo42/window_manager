use libc::{signal, SIGCHLD, SIG_DFL, SIG_IGN};
use std::io::Result;
use std::process::{Command, ExitStatus, Stdio};

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
pub fn split_commandline (commandline: &str) -> Vec<String> {
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
            result.push (elem);
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
    result.push (elem);
  }
  result
}

pub fn run (cmd: &[&str]) -> Result<()> {
  Command::new (cmd[0])
    .args (&cmd[1..])
    .spawn ()
    .and_then (|_| Ok (()))
}

pub fn run_and_await (cmd: &[&str]) -> Result<ExitStatus> {
  let _guard = Scoped_Default_SigChld::new ();
  Command::new (cmd[0])
    .args (&cmd[1..])
    .spawn ()
    .and_then (|mut c| c.wait ())
}

pub fn run_and_await_with_output (cmd: &[&str]) -> Result<String> {
  let _guard = Scoped_Default_SigChld::new ();
  Command::new (cmd[0])
    .args (&cmd[1..])
    .stderr (Stdio::inherit ())
    .output ()
    .map (|output| String::from_utf8_lossy (&output.stdout).into_owned ())
}
