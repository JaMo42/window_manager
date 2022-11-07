use super::notifications;
use super::bar;

pub mod actions {
  pub unsafe fn increase_volume () {
    super::change_volume (5);
  }

  pub unsafe fn decrease_volume () {
    super::change_volume (-5);
  }

  pub unsafe fn mute_volume () {
    super::mute_volume ();
    super::notify_volume (true);
  }
}

/// Executes `amixer get Master` and extracts whether it is muted and the volume level
pub fn get_volume_info () -> Option<(bool, u32)> {
  unsafe { libc::signal (libc::SIGCHLD, libc::SIG_DFL); }
  let result = match std::process::Command::new ("amixer")
      .args (["get", "Master"])
      .output () {
    Ok (raw_output) => {
      let output = String::from_utf8 (raw_output.stdout).unwrap ();
      // <level>%] [<on/off>]
      let info = &output[output.find ('[').unwrap () + 1 ..];
      let level = info.split ('%').next ().unwrap ().parse ().unwrap ();
      let muted = info.split ('[').nth (1).unwrap ().starts_with ("off]");
      Some ((muted, level))
    }
    Err (err) => {
      log::error! ("Command 'amixer get Master' failed: {}", err);
      None
    }
  };
  unsafe { libc::signal (libc::SIGCHLD, libc::SIG_IGN); }
  result
}

/// Sends a desktop notification about the current volume.
/// If `mute_notification` is `true` the notification states whether volume has
/// been muted or unmuted.
fn notify_volume (mute_notification: bool) {
  if let Some ((is_muted, level)) = get_volume_info () {
    let summary = if mute_notification {
      if is_muted {"Volume muted"} else {"Volume unmuted"}
    } else {
      "Volume level changed"
    };
    let body = if !mute_notification || !is_muted {
      format! ("volume level is {}%", level)
    } else {
      String::new ()
    };
    notifications::notify (summary, &body, 2000);
  } else {
    log::error! ("Failed to get volume information");
  }
}

/// Executes `amixer -q sset Master toggle`
fn mute_volume () {
  unsafe { libc::signal (libc::SIGCHLD, libc::SIG_DFL); }
  if let Ok (mut process) = std::process::Command::new ("amixer")
    .args (["-q", "sset", "Master", "toggle"])
    .spawn () {
    process.wait ().ok ();
  }
  unsafe { libc::signal (libc::SIGCHLD, libc::SIG_IGN); }
  bar::update ();
}

/// Executes `amixer -q sset Master [value]%[+/-] unmute`
fn change_volume (by: i32) {
  unsafe { libc::signal (libc::SIGCHLD, libc::SIG_IGN); }
  let arg = format! ("{}%{}", by.abs (), if by < 0 {'-'} else {'+'});
  if let Ok (mut process) = std::process::Command::new ("amixer")
    .args (["-q", "sset", "Master", &arg, "unmute"])
    .spawn () {
    process.wait ().ok ();
  }
  unsafe { libc::signal (libc::SIGCHLD, libc::SIG_IGN); }
  bar::update ();
}
