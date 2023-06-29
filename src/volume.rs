use crate::error::{LogError, LogNone};
use std::ffi::CStr;

#[allow(dead_code)] // will be used for the PulseAudio implementation
pub struct AppInfo {
    index: u32,
    name: String,
    icon_name: String,
    volume: f64,
}

pub trait AudioAPI {
    // Master controls
    fn master_volume(&self) -> u8;
    fn is_muted(&self) -> bool;
    fn mute_master(&self);
    fn increase_master_volume(&self, delta: u8);
    fn decrease_master_volume(&self, delta: u8);
    // Per-application controls, only provided by the PulseAudio backend
    fn list_apps(&self) -> Vec<AppInfo>;
    fn mute_app(&self, index: u32);
    fn increase_app_volume(&self, index: u32, delta: u8);
    fn decrease_app_volume(&self, index: u32, delta: u8);
}

pub struct ALSA {
    handle: alsa::Mixer,
    // `elem` lifetime is same as `handle`. We use `static` so we don't need to
    // deal with the borrow checker.
    elem: alsa::mixer::Selem<'static>,
    range: (i64, i64),
    range_div: i64,
}

impl ALSA {
    pub fn new() -> Option<Self> {
        let sid = alsa::mixer::SelemId::new("Master", 0);
        let mut handle = alsa::Mixer::open(false).log_error()?;
        handle
            .attach(unsafe { CStr::from_bytes_with_nul_unchecked("default\0".as_bytes()) })
            .log_error()?;
        alsa::mixer::Selem::register(&mut handle).log_error()?;
        handle.load().log_error()?;
        let elem: alsa::mixer::Selem<'static> = unsafe { &*(&handle as *const alsa::Mixer) }
            .find_selem(&sid)
            .log_none("ALSA: Unable to find simple control 'Master',0")?;
        let range = elem.get_playback_volume_range();
        Some(Self {
            handle,
            elem,
            range,
            range_div: (range.1 - range.0) / 100,
        })
    }
}

impl AudioAPI for ALSA {
    // TODO: don't `unwrap` the get and set functions maybe but idk why these
    //       would even fail at this point so it will probably stay like this
    //       until I get a crash from it.

    fn master_volume(&self) -> u8 {
        self.handle.handle_events().log_error();
        let channel = alsa::mixer::SelemChannelId::mono();
        let vol = self.elem.get_playback_volume(channel).unwrap();
        ((vol - self.range.0) / self.range_div) as u8
    }

    fn is_muted(&self) -> bool {
        self.handle.handle_events().log_error();
        let channel = alsa::mixer::SelemChannelId::mono();
        self.elem.get_playback_switch(channel).unwrap() == 0
    }

    fn mute_master(&self) {
        self.elem
            .set_playback_switch_all(if self.is_muted() { 1 } else { 0 })
            .unwrap();
    }

    fn increase_master_volume(&self, delta: u8) {
        let current = self.master_volume();
        let value = self.range.0 + (current as i64 + delta as i64) * self.range_div;
        self.elem
            .set_playback_volume_all(value.clamp(self.range.0, self.range.1))
            .unwrap();
    }

    fn decrease_master_volume(&self, delta: u8) {
        let current = self.master_volume();
        let value = self.range.0 + (current as i64 - delta as i64) * self.range_div;
        self.elem
            .set_playback_volume_all(value.clamp(self.range.0, self.range.1))
            .unwrap();
    }

    fn list_apps(&self) -> Vec<AppInfo> {
        Vec::new()
    }

    fn mute_app(&self, _: u32) {
        unimplemented!()
    }

    fn increase_app_volume(&self, _: u32, _: u8) {
        unimplemented!()
    }

    fn decrease_app_volume(&self, _: u32, _: u8) {
        unimplemented!()
    }
}

// TODO: PluseAudio implementation for per-application mixing but that only
// makes sense once we have a mixer gui.

pub fn get_audio_api() -> Option<Box<dyn AudioAPI>> {
    if let Some(alsa) = ALSA::new() {
        Some(Box::new(alsa))
    } else {
        None
    }
}
