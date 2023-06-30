use crate::{
    config::{Config, Size},
    draw::{load_builtin_svg, load_icon, Alignment, BuiltinResources, DrawingContext, Svg},
    error::OrFatal,
    event::{x_event_source, EventSink, Signal, SinkStorage},
    monitors::monitors,
    mouse_block::MouseBlock,
    rectangle::{Rectangle, ShowAt},
    update_thread::UpdateThread,
    volume::AppInfo,
    window_manager::{WindowKind, WindowManager},
    x::Window,
};
use pango::EllipsizeMode;
use parking_lot::Mutex;
use std::{borrow::Cow, sync::Arc};
use xcb::{x::EventMask, Event, Xid};

#[derive(Copy, Clone, Debug)]
struct ItemLayout {
    icon: Rectangle,
    title: Rectangle,
    minus: Rectangle,
    plus: Rectangle,
    mute: Rectangle,
    volume: Rectangle,
}

enum Icon {
    None,
    App(Svg),
    Builtin(Svg),
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
enum ClickHit {
    #[default]
    None,
    Minus,
    Plus,
    Mute,
}

impl ClickHit {
    fn into_option(self) -> Option<Self> {
        match self {
            Self::None => None,
            otherwise => Some(otherwise),
        }
    }
}

struct App {
    info: AppInfo,
    icon: Icon,
    layout: ItemLayout,
}

impl App {
    fn new(info: AppInfo, layout: ItemLayout, icon_theme: &str) -> Self {
        let icon = info
            .icon_name
            .as_deref()
            .map(|name| load_icon(name, icon_theme).map(Result::ok).flatten())
            .flatten()
            .map(Icon::App)
            .unwrap_or(Icon::None);
        Self { info, icon, layout }
    }

    fn paint(&self, config: &Config, dc: &DrawingContext, res: &BuiltinResources, hover: ClickHit) {
        let fg = config.colors.bar_text;
        match &self.icon {
            Icon::App(icon) => dc.draw_svg(icon, self.layout.icon),
            Icon::Builtin(icon) => dc.draw_colored_svg(icon, fg, self.layout.icon),
            Icon::None => {}
        }
        let title_str = if let Some(title) = &self.info.name {
            Cow::Borrowed(title.as_str())
        } else {
            Cow::Owned(format!("[Sink {}]", self.info.index))
        };
        let minus_color = if let ClickHit::Minus = hover {
            config.colors.focused
        } else {
            fg
        };
        let plus_color = if let ClickHit::Plus = hover {
            config.colors.focused
        } else {
            fg
        };
        let mute_color = if let ClickHit::Mute = hover {
            config.colors.focused
        } else {
            if self.info.is_muted {
                config.colors.urgent
            } else {
                fg
            }
        };
        dc.text(title_str.as_ref(), self.layout.title)
            .color(fg)
            .vertical_alignment(Alignment::CENTER)
            .ellipsize(EllipsizeMode::End)
            .draw();
        dc.draw_colored_svg(res.minus(), minus_color, self.layout.minus);
        dc.draw_colored_svg(res.plus(), plus_color, self.layout.plus);
        if self.info.is_muted {
            dc.draw_colored_svg(res.volume_muted(), mute_color, self.layout.mute);
        } else {
            dc.draw_colored_svg(res.volume(), mute_color, self.layout.mute);
        }
        dc.text(&format!("{}%", self.info.volume), self.layout.volume)
            .color(fg)
            .vertical_alignment(Alignment::CENTER)
            .horizontal_alignment(Alignment::RIGHT)
            .draw();
    }

    fn click(&self, x: i16, y: i16) -> ClickHit {
        if self.layout.minus.contains((x, y)) {
            ClickHit::Minus
        } else if self.layout.plus.contains((x, y)) {
            ClickHit::Plus
        } else if self.layout.mute.contains((x, y)) {
            ClickHit::Mute
        } else {
            ClickHit::None
        }
    }
}

#[derive(Debug)]
struct Layout {
    start_y: i16,
    line_height: i16,
    space: i16,
    icon_x: i16,
    title_x: i16,
    title_width: u16,
    minus_x: i16,
    plus_x: i16,
    mute_x: i16,
    volume_x: i16,
    volume_width: u16,
    window_width: u16,
}

impl Layout {
    fn compute(config: &Config, dc: &Arc<Mutex<DrawingContext>>) -> Self {
        let dpmm = monitors().primary().dpmm();
        let screen_size = *monitors().primary().geometry();
        let font_height = dc.lock().font_height(Some(&config.bar.font));
        let padding = Size::Physical(2.0).resolve(Some(dpmm), None, None) as i16;
        let line_height = Size::PercentOfFont(1.1).resolve(None, None, Some(font_height)) as i16;
        let space = Size::Physical(1.0).resolve(Some(dpmm), None, None) as i16;
        let title_width = config.bar.volume_mixer_title_width.resolve(
            Some(dpmm),
            Some(screen_size.width),
            Some(font_height),
        );
        let icon_x = padding;
        let title_x = icon_x + space + line_height;
        let minus_x = title_x + title_width as i16 + space;
        let plus_x = minus_x + line_height;
        let mute_x = plus_x + line_height;
        let volume_x = mute_x + space + line_height;
        let volume_width = dc.lock().text_width("100%", Some(&config.bar.font));
        let window_width = (volume_x + volume_width as i16 + padding) as u16;
        Self {
            start_y: padding,
            line_height,
            space,
            icon_x,
            title_x,
            title_width,
            plus_x,
            mute_x,
            minus_x,
            volume_x,
            volume_width,
            window_width,
        }
    }

    fn height_for(&self, item_count: usize) -> u16 {
        let item_count = item_count as i16;
        (2 * self.start_y + item_count * self.line_height + (item_count - 1) * self.space) as u16
    }

    fn item(&self, index: usize) -> ItemLayout {
        let y = self.start_y + index as i16 * (self.line_height + self.space);
        let size = self.line_height as u16;
        ItemLayout {
            icon: Rectangle::new(self.icon_x, y, size, size),
            title: Rectangle::new(self.title_x, y, self.title_width, size),
            plus: Rectangle::new(self.plus_x, y, size, size),
            minus: Rectangle::new(self.minus_x, y, size, size),
            mute: Rectangle::new(self.mute_x, y, size, size),
            volume: Rectangle::new(self.volume_x, y, self.volume_width, size),
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
struct HeldButton {
    app: Option<(usize, u32)>, // `None` for master volume
    button: ClickHit,
}

impl HeldButton {
    fn take(&mut self) -> Self {
        let res = *self;
        *self = Self::default();
        res
    }

    fn button_for_index(&self, index: Option<usize>) -> ClickHit {
        if self.app.map(|(i, _)| i) == index {
            self.button
        } else {
            ClickHit::None
        }
    }
}

struct VolumeMixer {
    wm: Arc<WindowManager>,
    master: App,
    apps: Vec<App>,
    window: Window,
    _layout: Layout,
    geometry: Rectangle,
    mouse_block: Option<MouseBlock>,
    hover: HeldButton,
    // The + and - buttons are handeled by an UpdateThread so they can be
    // repeatedly acticated while a button is held down.
    held_button: Option<HeldButton>,
    first_update: bool,
    ignore_updates: u64,
    update_thread: Option<UpdateThread>,
}

impl VolumeMixer {
    const HOLD_RATE: u64 = 75;
    const HOLD_DELAY: u64 = 375;

    fn new(wm: Arc<WindowManager>, at: ShowAt) -> Option<Self> {
        let mut ctl = wm.audio_api()?;
        let apps = ctl.list_apps();
        let master_volume = ctl.master_volume();
        let master_mute = ctl.is_muted();
        drop(ctl);
        let layout = Layout::compute(&wm.config, &wm.drawing_context);
        let visual = wm.display.truecolor_visual();
        let geometry = *at
            .translate((0, 0, layout.window_width, layout.height_for(apps.len() + 1)))
            .clamp_inside(monitors().primary().geometry());
        let window = Window::builder(wm.display.clone())
            .geometry(geometry)
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .cursor(wm.cursors.normal)
                    .background_pixel(0)
                    .border_pixel(0)
                    .event_mask(
                        EventMask::BUTTON_PRESS
                            | EventMask::BUTTON_RELEASE
                            | EventMask::POINTER_MOTION
                            | EventMask::KEY_PRESS
                            // We don't handle enter events but still want to get
                            // them to stop the main event sinks motion event
                            // compression from stealing our events.
                            | EventMask::ENTER_WINDOW,
                    )
                    .colormap(visual.colormap);
            })
            .build();
        wm.set_window_kind(&window, WindowKind::StatusBarWidget);
        log::trace!("volume mixer: window: {}", window.resource_id());

        let apps = apps
            .into_iter()
            .enumerate()
            .map(|(i, info)| {
                let item_layout = layout.item(i + 1);
                App::new(info, item_layout, &wm.config.icon_theme)
            })
            .collect();

        let master_layout = layout.item(0);
        let mut master = App::new(
            AppInfo {
                index: u32::MAX,
                name: Some("Master".to_string()),
                icon_name: None,
                volume: master_volume,
                is_muted: master_mute,
            },
            master_layout,
            &wm.config.icon_theme,
        );
        master.icon = Icon::Builtin(load_builtin_svg("volume"));

        Some(Self {
            wm,
            master,
            apps,
            window,
            _layout: layout,
            geometry,
            mouse_block: None,
            hover: HeldButton::default(),
            held_button: None,
            first_update: false,
            ignore_updates: 0,
            update_thread: None,
        })
    }

    fn update(&mut self, index: Option<usize>) {
        let mut ctl = self.wm.audio_api_unchecked();
        match index {
            None => {
                self.master.info.volume = ctl.master_volume();
                self.master.info.is_muted = ctl.is_muted();
            }
            Some(index) => {
                let app = &mut self.apps[index];
                ctl.update_app(&mut app.info);
            }
        }
        drop(ctl);
        self.paint();
        self.wm
            .signal_sender
            .send(Signal::UpdateBar(false))
            .or_fatal(&self.wm.display);
    }

    fn destroy(&mut self) {
        self.wm.remove_all_contexts(&self.window);
        if let Some(mouse_block) = &self.mouse_block {
            mouse_block.destroy(&self.wm);
        }
        if let Some(update_thread) = self.update_thread.take() {
            update_thread.stop();
        }
        self.window.destroy();
        self.wm.remove_event_sink(self.id());
    }

    fn show(&mut self) {
        log::trace!("showing volume mixer");
        self.window.map();
        if let Some(mouse_block) = &self.mouse_block {
            self.window.stack_above(mouse_block.handle());
        }
        self.paint();
        self.wm.display.set_input_focus(self.window.handle());
    }

    fn paint(&self) {
        let dc = self.wm.drawing_context.lock();
        let local_geometry = self.geometry.at(0, 0);
        dc.fill_rect(local_geometry, self.wm.config.colors.bar_background);
        self.master.paint(
            &self.wm.config,
            &dc,
            &self.wm.resources,
            self.hover.button_for_index(None),
        );
        for (i, app) in self.apps.iter().enumerate() {
            app.paint(
                &self.wm.config,
                &dc,
                &self.wm.resources,
                self.hover.button_for_index(Some(i)),
            );
        }
        dc.render(&self.window, local_geometry);
    }

    fn maybe_click(&mut self, x: i16, y: i16, app: Option<(usize, u32)>) -> bool {
        let a = match app {
            Some((index, _)) => &self.apps[index],
            None => &self.master,
        };
        match a.click(x, y) {
            ClickHit::Minus => {
                self.held_button = Some(HeldButton {
                    app,
                    button: ClickHit::Minus,
                });
                self.first_update = true;
                self.update_thread.as_mut().unwrap().update();
            }
            ClickHit::Plus => {
                self.held_button = Some(HeldButton {
                    app,
                    button: ClickHit::Plus,
                });
                self.first_update = true;
                self.update_thread.as_mut().unwrap().update();
            }
            ClickHit::Mute => {
                let mut ctl = self.wm.audio_api_unchecked();
                if app.is_none() {
                    ctl.mute_master();
                } else {
                    ctl.mute_app(a.info.index, !a.info.is_muted)
                }
            }
            ClickHit::None => return false,
        }
        self.update(app.map(|(index, _)| index));
        false
    }
}

impl EventSink for VolumeMixer {
    fn accept(&mut self, event: &xcb::Event) -> bool {
        let source = if let Some(source) = x_event_source(event) {
            source
        } else {
            return false;
        };
        match self.wm.get_window_kind(&source) {
            WindowKind::MouseBlock => self.destroy(),
            WindowKind::StatusBarWidget if source == self.window.handle() => {
                use xcb::x::Event::*;
                match event {
                    Event::X(ButtonPress(ev)) => {
                        if self.maybe_click(ev.event_x(), ev.event_y(), None) {
                            return true;
                        }
                        for i in 0..self.apps.len() {
                            let app = Some((i, self.apps[i].info.index));
                            if self.maybe_click(ev.event_x(), ev.event_y(), app) {
                                return true;
                            }
                        }
                    }
                    Event::X(ButtonRelease(_)) => {
                        self.held_button = None;
                    }
                    Event::X(MotionNotify(ev)) => {
                        let prev = self.hover.take();
                        let x = ev.event_x();
                        let y = ev.event_y();
                        if let Some(button) = self.master.click(x, y).into_option() {
                            self.hover = HeldButton { app: None, button };
                            if self.hover != prev {
                                self.paint();
                            }
                            return true;
                        }
                        for (i, app) in self.apps.iter().enumerate() {
                            if let Some(button) = app.click(x, y).into_option() {
                                self.hover = HeldButton {
                                    app: Some((i, 0)),
                                    button,
                                };
                                if self.hover != prev {
                                    self.paint();
                                }
                                return true;
                            }
                        }
                        if self.hover != prev {
                            self.paint();
                        }
                    }
                    Event::X(KeyPress(_)) => self.destroy(),
                    _ => {}
                }
            }
            _ => return false,
        }
        true
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[
            ButtonPressEvent::NUMBER,
            EnterNotifyEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
            MotionNotifyEvent::NUMBER,
            KeyPressEvent::NUMBER,
        ]
    }
}

pub fn volume_mixer(wm: Arc<WindowManager>, at: ShowAt) {
    if let Some(mixer) = VolumeMixer::new(wm.clone(), at) {
        let mixer = Arc::new(Mutex::new(mixer));
        let t_mixer = mixer.clone();
        let mouse_block = MouseBlock::new_invisible(&wm, monitors().primary());
        let mut lock = mixer.lock();
        lock.mouse_block = Some(mouse_block);
        lock.update_thread = Some(UpdateThread::new(VolumeMixer::HOLD_RATE, move || {
            let mut mixer = t_mixer.lock();
            if let Some(held) = mixer.held_button {
                // Handle the first update, then wait until the hold delay
                // before processing further updates.
                if mixer.first_update {
                    mixer.first_update = false;
                    mixer.ignore_updates = VolumeMixer::HOLD_DELAY / VolumeMixer::HOLD_RATE;
                } else if mixer.ignore_updates != 0 {
                    mixer.ignore_updates -= 1;
                    return;
                }
                let mut ctl = mixer.wm.audio_api_unchecked();
                if let Some((index, app_index)) = held.app {
                    match held.button {
                        ClickHit::Minus => ctl.decrease_app_volume(app_index, 5),
                        ClickHit::Plus => ctl.increase_app_volume(app_index, 5),
                        _ => unreachable!(),
                    }
                    drop(ctl);
                    mixer.update(Some(index));
                } else {
                    match held.button {
                        ClickHit::Minus => ctl.decrease_master_volume(5),
                        ClickHit::Plus => ctl.increase_master_volume(5),
                        _ => unreachable!(),
                    }
                    drop(ctl);
                    mixer.update(None);
                }
            }
        }));
        lock.show();
        drop(lock);
        wm.add_event_sink(SinkStorage::Mutex(mixer));
    } else {
        log::info!("could not create volume mixer (no backend available)");
    }
}
