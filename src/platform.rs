use super::bar;
use super::notifications;
use super::process::{run_and_await, run_and_await_with_output};

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
  let result = match run_and_await_with_output (&["amixer", "get", "Master"]) {
    Ok (output) => {
      // <level>%] [<on/off>]
      let info = &output[output.find ('[').unwrap () + 1..];
      let level = info.split ('%').next ().unwrap ().parse ().unwrap ();
      let muted = info.split ('[').nth (1).unwrap ().starts_with ("off]");
      Some ((muted, level))
    }
    Err (err) => {
      log::error! ("Command 'amixer get Master' failed: {}", err);
      None
    }
  };
  result
}

/// Sends a desktop notification about the current volume.
/// If `mute_notification` is `true` the notification states whether volume has
/// been muted or unmuted.
fn notify_volume (mute_notification: bool) {
  if let Some ((is_muted, level)) = get_volume_info () {
    let summary = if mute_notification {
      if is_muted {
        "Volume muted"
      } else {
        "Volume unmuted"
      }
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
  run_and_await (&["amixer", "-q", "sset", "Master", "toggle"]).ok ();
  bar::update ();
}

/// Executes `amixer -q sset Master [value]%[+/-] unmute`
fn change_volume (by: i32) {
  let arg = format! ("{}%{}", by.abs (), if by < 0 { '-' } else { '+' });
  run_and_await (&["amixer", "-q", "sset", "Master", &arg, "unmute"]).ok ();
  bar::update ();
}

pub fn suspend () -> std::io::Result<()> {
  run_and_await (&["systemctl", "suspend"]).map (|_| ())
}

pub fn logout () -> std::io::Result<()> {
  // An empty argument terminates the calling session
  run_and_await (&["loginctl", "terminate-session", ""]).map (|_| ())
}
