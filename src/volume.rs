use crate::config::Config;

pub struct AppInfo {
    pub indices: Vec<u32>,
    pub name: Option<String>,
    pub icon_name: Option<String>,
    pub volume: u8,
    pub is_muted: bool,
}

pub trait AudioAPI {
    // Master controls
    fn master_volume(&mut self) -> u8;
    fn is_muted(&mut self) -> bool;
    fn mute_master(&mut self);
    fn increase_master_volume(&mut self, delta: u8);
    fn decrease_master_volume(&mut self, delta: u8);
    // Per-application controls, only provided by the PulseAudio backend
    fn list_apps(&mut self) -> Vec<AppInfo>;
    fn update_app(&mut self, app: &mut AppInfo);
    fn mute_app(&mut self, app: &AppInfo);
    fn increase_app_volume(&mut self, app: &AppInfo, delta: u8);
    fn decrease_app_volume(&mut self, app: &AppInfo, delta: u8);
}

#[cfg(feature = "my_alsa")]
mod my_alsa {
    use super::*;
    use crate::error::{LogError, LogNone};
    use alsa::{
        mixer::{Selem, SelemChannelId, SelemId},
        Mixer,
    };
    use std::ffi::CStr;

    pub struct Alsa {
        handle: Mixer,
        // `elem` lifetime is same as `handle`. We use `static` so we don't need to
        // deal with the borrow checker.
        elem: Selem<'static>,
        range: (i64, i64),
        range_div: i64,
    }

    impl Alsa {
        pub fn new() -> Option<Self> {
            let sid = SelemId::new("Master", 0);
            let mut handle = Mixer::open(false).log_error()?;
            handle
                .attach(unsafe { CStr::from_bytes_with_nul_unchecked("default\0".as_bytes()) })
                .log_error()?;
            Selem::register(&mut handle).log_error()?;
            handle.load().log_error()?;
            let elem: Selem<'static> = unsafe { &*(&handle as *const Mixer) }
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

    impl AudioAPI for Alsa {
        // TODO: don't `unwrap` the get and set functions maybe but idk why these
        //       would even fail at this point so it will probably stay like this
        //       until I get a crash from it.

        fn master_volume(&mut self) -> u8 {
            self.handle.handle_events().log_error();
            let channel = SelemChannelId::mono();
            let vol = self.elem.get_playback_volume(channel).unwrap();
            ((vol - self.range.0) / self.range_div) as u8
        }

        fn is_muted(&mut self) -> bool {
            self.handle.handle_events().log_error();
            let channel = SelemChannelId::mono();
            self.elem.get_playback_switch(channel).unwrap() == 0
        }

        fn mute_master(&mut self) {
            let is_muted = self.is_muted();
            self.elem
                .set_playback_switch_all(if is_muted { 1 } else { 0 })
                .unwrap();
        }

        fn increase_master_volume(&mut self, delta: u8) {
            let current = self.master_volume();
            let value = self.range.0 + (current as i64 + delta as i64) * self.range_div;
            self.elem
                .set_playback_volume_all(value.clamp(self.range.0, self.range.1))
                .unwrap();
        }

        fn decrease_master_volume(&mut self, delta: u8) {
            let current = self.master_volume();
            let value = self.range.0 + (current as i64 - delta as i64) * self.range_div;
            self.elem
                .set_playback_volume_all(value.clamp(self.range.0, self.range.1))
                .unwrap();
        }

        fn list_apps(&mut self) -> Vec<AppInfo> {
            Vec::new()
        }

        fn update_app(&mut self, _: &mut AppInfo) {
            unimplemented!()
        }

        fn mute_app(&mut self, _: &AppInfo) {
            unimplemented!()
        }

        fn increase_app_volume(&mut self, _: &AppInfo, _: u8) {
            unimplemented!()
        }

        fn decrease_app_volume(&mut self, _: &AppInfo, _: u8) {
            unimplemented!()
        }
    }
}

#[cfg(feature = "pulse")]
mod pulse {
    use super::*;
    use crate::error::LogError;
    use libpulse_binding::volume::{ChannelVolumes, Volume};
    use pulsectl::controllers::{
        types::ApplicationInfo, AppControl, DeviceControl, SinkController,
    };
    use std::{
        collections::{hash_map::DefaultHasher, HashMap},
        hash::{Hash, Hasher},
        ops::Add,
    };

    impl AppInfo {
        fn new(instances: Vec<ApplicationInfo>) -> Self {
            let mut indices = Vec::with_capacity(instances.len());
            let mut volume = u8::MAX;
            let mut is_muted = false;
            let mut name = None;
            let mut icon_name = None;
            for i in instances {
                indices.push(i.index);
                volume = volume.min(PulseAudio::volume2percent(i.volume.avg()));
                is_muted = is_muted || i.mute;
                if name.is_none() {
                    if let Some(a_name) = i.proplist.get_str("application.name") {
                        name = Some(a_name);
                    }
                }
                if icon_name.is_none() {
                    if let Some(a_icon_name) = i.proplist.get_str("application.icon_name") {
                        icon_name = Some(a_icon_name);
                    }
                }
            }
            Self {
                indices,
                name,
                icon_name,
                volume,
                is_muted,
            }
        }
    }

    pub struct PulseAudio {
        handler: SinkController,
        default_device: u32,
        group_key: &'static str,
    }

    impl PulseAudio {
        pub fn new(config: &Config) -> Option<Self> {
            let mut handler = SinkController::create().log_error()?;
            let default_device = handler.get_default_device().log_error()?.index;
            Some(Self {
                handler,
                default_device,
                group_key: match config.bar.volume_mixer_grouping.as_str() {
                    "name" => "application.name",
                    _ => "application.process.id",
                },
            })
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
        fn master_volume(&mut self) -> u8 {
            Self::volume2percent(self.handler.get_default_device().unwrap().volume.avg())
        }

        fn is_muted(&mut self) -> bool {
            self.handler.get_default_device().unwrap().mute
        }

        fn mute_master(&mut self) {
            let is_muted = self.is_muted();
            self.handler
                .set_device_mute_by_index(self.default_device, !is_muted)
        }

        fn increase_master_volume(&mut self, delta: u8) {
            let mut dev = self.handler.get_default_device().unwrap();
            let new_volume = self.change_volume(&mut dev.volume, delta, u32::add);
            self.handler
                .set_device_volume_by_index(self.default_device, new_volume);
        }

        fn decrease_master_volume(&mut self, delta: u8) {
            let mut dev = self.handler.get_default_device().unwrap();
            let new_volume = self.change_volume(&mut dev.volume, delta, u32::saturating_sub);
            self.handler
                .set_device_volume_by_index(self.default_device, new_volume);
        }

        fn list_apps(&mut self) -> Vec<AppInfo> {
            let mut apps = HashMap::new();
            for app in self.handler.list_applications().unwrap() {
                let mut hasher = DefaultHasher::new();
                app.proplist.get(self.group_key).hash(&mut hasher);
                // In addition to the group key we also want to make sure the
                // instances start with the same state, for the mute state this is
                // not that important since we set it to an absulute value anyways
                // but the volume is only changed by a delta so it would always
                // stay de-synced.  We could synchronize it manually but potentially
                // changing the values of programs just because the volume mixer was
                // opened seem like a bad idea.
                app.mute.hash(&mut hasher);
                Self::volume2percent(app.volume.avg()).hash(&mut hasher);
                let id = hasher.finish();
                apps.entry(id)
                    .or_insert_with(|| Vec::with_capacity(1))
                    .push(app);
            }
            apps.into_values().map(AppInfo::new).collect()
        }

        fn update_app(&mut self, app: &mut AppInfo) {
            if let Ok(info) = self.handler.get_app_by_index(app.indices[0]) {
                app.is_muted = info.mute;
                app.volume = Self::volume2percent(info.volume.avg());
            }
        }

        fn mute_app(&mut self, app: &AppInfo) {
            for &index in app.indices.iter() {
                self.handler.set_app_mute(index, !app.is_muted).log_error();
            }
        }

        fn increase_app_volume(&mut self, app: &AppInfo, delta: u8) {
            let delta = delta as f64 / 100.0;
            for &index in app.indices.iter() {
                self.handler.increase_app_volume_by_percent(index, delta);
            }
        }

        fn decrease_app_volume(&mut self, app: &AppInfo, delta: u8) {
            let delta = delta as f64 / 100.0;
            for &index in app.indices.iter() {
                self.handler.decrease_app_volume_by_percent(index, delta);
            }
        }
    }
}

pub fn get_audio_api(_config: &Config) -> Option<Box<dyn AudioAPI>> {
    #[cfg(feature = "pulse")]
    if let Some(pulseaudio) = pulse::PulseAudio::new(_config) {
        log::info!("using PulseAudio backend");
        return Some(Box::new(pulseaudio));
    }
    #[cfg(feature = "my_alsa")]
    if let Some(alsa) = my_alsa::Alsa::new() {
        log::info!("using ALSA backend");
        return Some(Box::new(alsa));
    }
    None
}
