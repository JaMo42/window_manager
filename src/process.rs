use crate::{error::message_box, AnyResult};
use libc::{_exit, c_void, close, read, setsid, waitpid, write};
use parking_lot::Mutex;
use std::{
    io::Result,
    mem::zeroed,
    process::{Command, ExitStatus, Stdio},
};

#[derive(Clone, Debug)]
struct AnonPipe {
    tx: i32,
    rx: i32,
}

impl AnonPipe {
    fn new() -> AnonPipe {
        let mut fds = [0; 2];
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        AnonPipe {
            tx: fds[1],
            rx: fds[0],
        }
    }

    /// Returns the ends of the pipe as `(write_end, read_end)`.
    fn ends(&self) -> (i32, i32) {
        (self.tx, self.rx)
    }
}

impl Drop for AnonPipe {
    fn drop(&mut self) {
        unsafe {
            close(self.rx);
            close(self.tx);
        }
    }
}

/// Spawns the given command as an orphan.
/// This command will be a child of the init command and we do not need to await
/// it in order to clean resources.
/// However we cannot communicate with the process either.
fn orphan(mut command: Command) -> Result<()> {
    const RESULT_SIZE: usize = std::mem::size_of::<Result<()>>();
    static PIPE: Mutex<Option<AnonPipe>> = Mutex::new(None);
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut pipe_lock = PIPE.lock();
    let (rx, tx) = pipe_lock.get_or_insert_with(AnonPipe::new).ends();
    unsafe {
        let pid = libc::fork();
        if pid == 0 {
            close(rx);
            setsid();
            let result = command.spawn().map(|_| ());
            write(
                tx,
                &result as *const std::io::Result<()> as *const c_void,
                RESULT_SIZE,
            );
            close(tx);
            _exit(0);
        }
        waitpid(pid, std::ptr::null_mut(), 0);
        let mut child = zeroed();
        read(
            rx,
            &mut child as *mut std::io::Result<()> as *mut c_void,
            RESULT_SIZE,
        );
        child
    }
}

/// Splits a commandline into it's elements, handling strings and escaped
/// characters. Strings are delimited by either `'` or `"`, any character after
/// a `\` is ignored.
pub fn split_commandline<S>(commandline: &str) -> Vec<S>
where
    S: std::str::FromStr,
{
    let mut result = Vec::new();
    let mut elem = String::new();
    let mut in_string: char = '\0';
    let mut ignore = false;
    for c in commandline.chars() {
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
                    elem.push(c);
                }
            }
            ' ' => {
                if in_string == '\0' {
                    if !elem.is_empty() {
                        result.push(unsafe { S::from_str(&elem).unwrap_unchecked() });
                        elem = String::new();
                    }
                } else {
                    elem.push(c);
                }
            }
            _ => {
                elem.push(c);
            }
        }
    }
    if !elem.is_empty() {
        result.push(unsafe { S::from_str(&elem).unwrap_unchecked() });
    }
    result
}

pub fn run(cmd: &[impl AsRef<str>]) -> AnyResult<()> {
    let mut command = Command::new(cmd[0].as_ref());
    command.args(cmd[1..].iter().map(|a| a.as_ref()));
    Ok(orphan(command)?)
}

pub fn run_or_message_box(cmd: &[impl AsRef<str>]) {
    if let Err(error) = run(cmd) {
        let commandline = cmd.iter().fold(String::new(), |a, b| a + " " + b.as_ref());
        let body = format!("{}\n{}", commandline, error);
        message_box("Failed to run process:", &body);
    }
}

pub fn run_and_await(cmd: &[&str]) -> AnyResult<ExitStatus> {
    Ok(Command::new(cmd[0])
        .args(&cmd[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?)
}

#[allow(dead_code)]
pub fn run_and_await_with_output(cmd: &[&str]) -> AnyResult<String> {
    Ok(Command::new(cmd[0])
        .args(&cmd[1..])
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())?)
}
