use crate::error::{LogError, LogNone};
use libpulse_binding::volume::{ChannelVolumes, Volume};
use pulsectl::controllers::{types::ApplicationInfo, AppControl, DeviceControl, SinkController};
use std::{
    cell::{RefCell, RefMut},
    ffi::CStr,
    ops::Add,
};

pub struct AppInfo {
    pub index: u32,
    pub name: Option<String>,
    pub icon_name: Option<String>,
    pub volume: u8,
    pub is_muted: bool,
}

impl AppInfo {
    fn from_pa(info: ApplicationInfo) -> Self {
        let name = info.proplist.get_str("application.name");
        let icon_name = info.proplist.get_str("application.icon_name");
        Self {
            index: info.index,
            name,
            icon_name,
            volume: PulseAudio::volume2percent(info.volume.avg()),
            is_muted: info.mute,
        }
    }
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
    fn update_app(&self, app: &mut AppInfo);
    fn mute_app(&self, index: u32, mute: bool);
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
        let is_muted = self.is_muted();
        self.elem
            .set_playback_switch_all(if is_muted { 1 } else { 0 })
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

    fn update_app(&self, _: &mut AppInfo) {
        unimplemented!()
    }

    fn mute_app(&self, _: u32, _: bool) {
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

pub struct PulseAudio {
    handler: RefCell<SinkController>,
    default_device: u32,
}

impl PulseAudio {
    pub fn new() -> Option<Self> {
        let mut handler = SinkController::create().log_error()?;
        let default_device = handler.get_default_device().log_error()?;
        let default_device_index = default_device.index;
        Some(Self {
            handler: RefCell::new(handler),
            default_device: default_device_index,
        })
    }

    fn handler_mut(&self) -> RefMut<SinkController> {
        self.handler.borrow_mut()
    }

    fn volume2percent(volume: Volume) -> u8 {
        (volume.0 / (Volume::NORMAL.0 / 100)) as u8
    }

    fn percent2volume(percent: u8) -> Volume {
        Volume(percent as u32 * (Volume::NORMAL.0 / 100))
    }

    fn change_volume<'a>(
        &self,
        channels: &'a mut ChannelVolumes,
        delta: u8,
        op: fn(u32, u32) -> u32,
    ) -> &'a ChannelVolumes {
        let old_inner = channels.avg().0;
        let new_inner = op(old_inner, Self::percent2volume(delta).0).min(Volume::NORMAL.0);
        let volume = Volume(new_inner);
        channels.set(channels.len(), volume)
    }
}

impl AudioAPI for PulseAudio {
    fn master_volume(&self) -> u8 {
        Self::volume2percent(
            self.handler_mut()
                .get_default_device()
                .unwrap()
                .volume
                .avg(),
        )
    }

    fn is_muted(&self) -> bool {
        self.handler_mut().get_default_device().unwrap().mute
    }

    fn mute_master(&self) {
        let is_muted = self.is_muted();
        self.handler_mut()
            .set_device_mute_by_index(self.default_device, !is_muted)
    }

    fn increase_master_volume(&self, delta: u8) {
        let mut dev = self.handler_mut().get_default_device().unwrap();
        let new_volume = self.change_volume(&mut dev.volume, delta, u32::add);
        self.handler_mut()
            .set_device_volume_by_index(self.default_device, new_volume);
    }

    fn decrease_master_volume(&self, delta: u8) {
        let mut dev = self.handler_mut().get_default_device().unwrap();
        let new_volume = self.change_volume(&mut dev.volume, delta, u32::saturating_sub);
        self.handler_mut()
            .set_device_volume_by_index(self.default_device, new_volume);
    }

    fn list_apps(&self) -> Vec<AppInfo> {
        self.handler_mut()
            .list_applications()
            .unwrap()
            .into_iter()
            .map(AppInfo::from_pa)
            .collect()
    }

    fn update_app(&self, app: &mut AppInfo) {
        if let Ok(info) = self.handler_mut().get_app_by_index(app.index) {
            app.is_muted = info.mute;
            app.volume = Self::volume2percent(info.volume.avg());
        }
    }

    fn mute_app(&self, index: u32, mute: bool) {
        self.handler_mut().set_app_mute(index, mute).log_error();
    }

    fn increase_app_volume(&self, index: u32, delta: u8) {
        let delta = delta as f64 / 100.0;
        self.handler_mut()
            .increase_app_volume_by_percent(index, delta);
    }

    fn decrease_app_volume(&self, index: u32, delta: u8) {
        let delta = delta as f64 / 100.0;
        self.handler_mut()
            .decrease_app_volume_by_percent(index, delta);
    }
}

pub fn get_audio_api() -> Option<Box<dyn AudioAPI>> {
    if let Some(pulseaudio) = PulseAudio::new() {
        log::info!("using PulseAudio backend");
        Some(Box::new(pulseaudio))
    } else if let Some(alsa) = ALSA::new() {
        log::info!("using ALSA backend");
        Some(Box::new(alsa))
    } else {
        None
    }
}
