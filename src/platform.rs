use crate::{process::run_and_await, window_manager::WindowManager, AnyResult};

pub mod actions {
    use crate::window_manager::WindowManager;

    pub fn increase_volume(wm: &WindowManager) {
        if let Some(ctl) = &wm.audio_api {
            ctl.increase_master_volume(5);
        }
    }

    pub fn decrease_volume(wm: &WindowManager) {
        if let Some(ctl) = &wm.audio_api {
            ctl.decrease_master_volume(5);
        }
    }

    pub fn mute_volume(wm: &WindowManager) {
        if let Some(ctl) = &wm.audio_api {
            ctl.mute_master();
            super::notify_volume(wm, true);
        }
    }
}

/// Sends a desktop notification about the current volume.
/// If `mute_notification` is `true` the notification states whether volume has
/// been muted or unmuted.
fn notify_volume(wm: &WindowManager, mute_notification: bool) {
    if let Some(ctl) = &wm.audio_api {
        let is_muted = ctl.is_muted();
        let level = ctl.master_volume();
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
            format!("volume level is {}%", level)
        } else {
            String::new()
        };
        let icon = if is_muted {
            "notification-audio-volume-muted"
        } else {
            "notification-audio-volume-high"
        };
        wm.notify(summary, &body, icon, 2000);
    }
}

pub fn suspend() -> AnyResult<()> {
    run_and_await(&["systemctl", "suspend"]).map(|_| ())
}

pub fn logout() -> AnyResult<()> {
    // An empty argument terminates the calling session
    run_and_await(&["loginctl", "terminate-session", ""]).map(|_| ())
}
