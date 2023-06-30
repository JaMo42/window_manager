use crate::{
    action::quit_dialog,
    bar::volume_mixer::volume_mixer,
    color::Color,
    config::Config,
    draw::{Alignment, ColorKind, DrawingContext, Svg},
    error::OrFatal,
    event::Signal,
    ewmh::{self, WindowType},
    mouse::{BUTTON_1, BUTTON_2, BUTTON_3, BUTTON_4, BUTTON_5},
    rectangle::{Rectangle, ShowAt},
    tooltip::Tooltip,
    window_manager::{WindowKind, WindowManager},
    x::Window,
};
use cairo::LineCap;
use chrono::{Local, Locale};
use parking_lot::MutexGuard;
use std::{rc::Rc, sync::Arc};
use xcb::x::{ButtonPressEvent, EventMask};

fn get_time_locale() -> Option<Locale> {
    let lc_time = std::env::var("LC_TIME").ok()?;
    let without_encoding = lc_time.split('.').next().unwrap();
    Locale::try_from(without_encoding).ok()
}

fn create_window(bar_window: &Window, wm: &WindowManager) -> Window {
    let display = bar_window.display().clone();
    let visual = *display.truecolor_visual();
    let window = Window::builder(display)
        .parent(bar_window.handle())
        .depth(visual.depth)
        .visual(visual.id)
        .attributes(|attributes| {
            attributes
                .override_redirect()
                .colormap(visual.colormap)
                .background_pixel(wm.config.colors.bar_background.pack())
                .border_pixel(0)
                .event_mask(
                    EventMask::BUTTON_PRESS | EventMask::ENTER_WINDOW | EventMask::LEAVE_WINDOW,
                );
        })
        .build();
    ewmh::set_window_type(&window, WindowType::Dock);
    wm.set_window_kind(&window, WindowKind::StatusBar);
    window.map();
    window
}

fn draw_icon_and_text(
    dc: &MutexGuard<DrawingContext>,
    string: &str,
    icon: Option<&Svg>,
    color: Color,
    icon_text_gap: u16,
    x: i16,
    height: u16,
) -> u16 {
    let mut width = 0;
    if let Some(icon) = icon {
        let size = (height as f64 * 0.9).round() as u16;
        let pos = (height - size) as i16 / 2;
        let rect = Rectangle::new(x + pos, pos, size, size);
        dc.draw_colored_svg(icon, color, rect.scale(90));
        width += height;
    }
    if string.is_empty() {
        return width;
    }
    width += icon_text_gap;
    width += dc
        .text(string, (x + width as i16, 0, 0, height))
        .color(color)
        .vertical_alignment(Alignment::CENTER)
        .draw()
        .width;
    width
}

#[derive(Copy, Clone, Debug)]
pub enum UpdateResult {
    New(u16),
    Old(u16),
}

impl UpdateResult {
    pub fn width(self) -> u16 {
        match self {
            Self::New(width) => width,
            Self::Old(width) => width,
        }
    }

    pub fn is_new(&self) -> bool {
        matches!(self, Self::New(_))
    }
}

pub trait Widget {
    fn resize(&self, rect: Rectangle) {
        self.window().move_and_resize(rect);
    }

    fn window(&self) -> &Window;

    /// Should draw the context to the context with the given height and X
    /// position and return the width drawn.
    fn update(&mut self, dc: &MutexGuard<DrawingContext>, height: u16, x: i16) -> UpdateResult;

    fn click(&mut self, _: &ButtonPressEvent) {}

    fn enter(&mut self) {}

    fn leave(&mut self) {}

    /// Should invalidate the widget state so the next call `update` always
    /// redraws it.
    fn invalidate(&mut self) {}

    fn signal(&mut self, _: &Signal) {}

    fn set_geometry(&mut self, _: Rectangle) {}
}

pub struct Workspaces {
    window: Window,
    wm: Arc<WindowManager>,
    workspace: usize,
    urgent: Vec<usize>,
    need_redraw: bool,
    height: u16,
}

impl Workspaces {
    pub fn new(bar_window: &Window, wm: &Arc<WindowManager>) -> Option<Self> {
        Some(Self {
            window: create_window(bar_window, wm),
            wm: wm.clone(),
            workspace: wm.active_workspace().index(),
            urgent: vec![0; wm.config.layout.workspaces],
            need_redraw: true,
            height: 0,
        })
    }
}

impl Widget for Workspaces {
    fn window(&self) -> &Window {
        &self.window
    }

    fn update(&mut self, dc: &MutexGuard<DrawingContext>, height: u16, mut x: i16) -> UpdateResult {
        if !self.need_redraw {
            return UpdateResult::Old(height * self.urgent.len() as u16);
        }
        for (idx, urgent) in self.urgent.iter().cloned().enumerate() {
            let color = if idx == self.workspace {
                self.wm.config.colors.focused_border()
            } else if urgent > 0 {
                self.wm.config.colors.urgent_border()
            } else {
                self.wm.config.colors.bar()
            };
            // Don't draw the outline for the background color
            if color.kind() == super::COLOR_KIND {
                dc.fill_rect((x, 0, height, height), color.border());
            } else {
                dc.rect((x, 0, height, height))
                    .color(color.border())
                    .stroke(2, ColorKind::Solid(color.border().scale(0.8)))
                    .draw();
            }
            dc.text(&format!("{}", idx + 1), (x, 0, height, height))
                .horizontal_alignment(Alignment::CENTER)
                .vertical_alignment(Alignment::CENTER)
                .color(color.text())
                .draw();
            x += height as i16;
        }
        self.height = height;
        self.need_redraw = false;
        UpdateResult::New(height * self.urgent.len() as u16)
    }

    fn click(&mut self, event: &ButtonPressEvent) {
        use crate::action::select_workspace;
        let button = event.detail();
        let idx = (event.event_x() as u16 / self.height) as usize;
        if idx >= self.urgent.len() {
            return;
        }
        if button == BUTTON_1 || button == BUTTON_2 || button == BUTTON_3 {
            // Left/Middle/Right click selects workspace under cursor
            select_workspace(&self.wm, idx, None);
        } else if button == BUTTON_5 {
            // Scrolling up selects the next workspace
            select_workspace(&self.wm, (self.workspace + 1) % self.urgent.len(), None)
        } else if button == BUTTON_4 {
            // Scrolling down selects the previous workspace
            if self.workspace == 0 {
                select_workspace(&self.wm, self.urgent.len() - 1, None);
            } else {
                select_workspace(&self.wm, self.workspace - 1, None);
            }
        }
    }

    fn invalidate(&mut self) {
        self.need_redraw = true;
    }

    fn signal(&mut self, signal: &Signal) {
        match signal {
            Signal::WorkspaceChanged(_, to) => {
                self.workspace = *to;
                self.need_redraw = true;
            }
            Signal::UrgencyChanged(handle) => {
                let client = self.wm.win2client(handle).unwrap();
                let workspace = client.workspace();
                if client.is_urgent() {
                    self.urgent[workspace] += 1;
                } else if self.urgent[workspace] > 0 {
                    // TODO: this seems kinda weird, maybe store a vector of
                    // urgent clients instead of the count.
                    self.urgent[workspace] -= 1;
                }
                self.need_redraw = true;
            }
            _ => {}
        }
    }
}

pub struct DateTime {
    window: Window,
    config: Arc<Config>,
    icon: Rc<Svg>,
    last_label: String,
    width: u16,
    locale: Option<Locale>,
}

impl DateTime {
    pub fn new(bar_window: &Window, wm: &WindowManager) -> Option<Self> {
        Some(Self {
            window: create_window(bar_window, wm),
            config: wm.config.clone(),
            icon: wm.resources.calendar().clone(),
            last_label: String::new(),
            width: 0,
            locale: get_time_locale().filter(|_| wm.config.bar.localized_time),
        })
    }
}

impl Widget for DateTime {
    fn window(&self) -> &Window {
        &self.window
    }

    fn update(&mut self, dc: &MutexGuard<DrawingContext>, height: u16, x: i16) -> UpdateResult {
        let now = Local::now();
        let label = match self.locale {
            Some(locale) => format!(
                "{}",
                now.format_localized(&self.config.bar.time_format, locale)
            ),
            None => format!("{}", now.format(&self.config.bar.time_format)),
        };
        if label == self.last_label {
            return UpdateResult::Old(self.width);
        }
        self.width = draw_icon_and_text(
            dc,
            &label,
            Some(&self.icon),
            self.config.colors.bar_text,
            3,
            x,
            height,
        );
        self.last_label = label;
        UpdateResult::New(self.width)
    }

    fn invalidate(&mut self) {
        self.last_label.clear();
    }

    fn click(&mut self, _: &ButtonPressEvent) {
        if self.config.bar.localized_time {
            self.locale = get_time_locale();
            self.invalidate();
        }
    }
}

pub struct Volume {
    window: Window,
    normal_icon: Rc<Svg>,
    muted_icon: Rc<Svg>,
    color: Color,
    wm: Arc<WindowManager>,
    last_level: u32,
    last_mute_state: Option<bool>,
    width: u16,
    geometry: Rectangle,
}

impl Volume {
    pub fn new(bar_window: &Window, wm: &Arc<WindowManager>) -> Option<Self> {
        if wm.audio_api.is_some() {
            Some(Self {
                window: create_window(bar_window, wm),
                normal_icon: wm.resources.volume().clone(),
                muted_icon: wm.resources.volume_muted().clone(),
                color: wm.config.colors.bar_text,
                wm: wm.clone(),
                last_level: u32::MAX,
                last_mute_state: None,
                width: 0,
                geometry: Rectangle::zeroed(),
            })
        } else {
            None
        }
    }
}

impl Widget for Volume {
    fn window(&self) -> &Window {
        &self.window
    }

    fn update(&mut self, dc: &MutexGuard<DrawingContext>, height: u16, x: i16) -> UpdateResult {
        let ctl = unsafe { self.wm.audio_api.as_ref().unwrap_unchecked() };
        let is_muted = ctl.is_muted();
        let level = ctl.master_volume() as u32;
        if level == self.last_level && self.last_mute_state.map_or(false, |last| is_muted == last) {
            return UpdateResult::Old(self.width);
        }
        let width = if is_muted {
            draw_icon_and_text(
                dc,
                "muted",
                Some(&self.muted_icon),
                self.color,
                3,
                x,
                height,
            )
        } else {
            let label = format!("{}%", level);
            draw_icon_and_text(
                dc,
                &label,
                Some(&self.normal_icon),
                self.color,
                3,
                x,
                height,
            )
        };
        self.last_level = level;
        self.last_mute_state = Some(is_muted);
        self.width = width;
        UpdateResult::New(width)
    }

    fn click(&mut self, event: &ButtonPressEvent) {
        use crate::platform::actions::*;
        let button = event.detail();
        if button == BUTTON_1 {
            mute_volume(&self.wm);
        } else if button == BUTTON_3 {
            let x = self.geometry.x + self.geometry.width as i16 / 2;
            let y = self.geometry.y + self.geometry.height as i16;
            volume_mixer(self.wm.clone(), ShowAt::TopCenter((x, y)));
        } else if button == BUTTON_5 {
            // Scroll up
            increase_volume(&self.wm);
        } else if button == BUTTON_4 {
            // Scroll down
            decrease_volume(&self.wm);
        }
    }

    fn invalidate(&mut self) {
        self.last_mute_state = None;
    }

    fn set_geometry(&mut self, geometry: Rectangle) {
        self.geometry = geometry;
    }
}

pub struct Battery {
    window: Window,
    config: Arc<Config>,
    hover_text: String,
    last_capacity: u32,
    last_status: String,
    width: u16,
    tooltip: Tooltip,
    geometry: Rectangle,
}

impl Battery {
    pub fn new(bar_window: &Window, wm: &Arc<WindowManager>) -> Option<Self> {
        if std::fs::metadata(format!(
            "/sys/class/power_supply/{}",
            wm.config.bar.power_supply
        ))
        .is_ok()
        {
            Some(Self {
                window: create_window(bar_window, wm),
                config: wm.config.clone(),
                hover_text: String::new(),
                last_capacity: u32::MAX,
                last_status: String::new(),
                width: 0,
                tooltip: Tooltip::new(wm),
                geometry: Rectangle::zeroed(),
            })
        } else {
            None
        }
    }

    fn draw_icon(&self, dc: &MutexGuard<DrawingContext>, x: i16, height: u16) -> u16 {
        const OUTLINE_WIDTH: i16 = 2;
        const INNER_GAP_WIDTH: i16 = 2;
        const CONNECTOR_WIDTH: i16 = 3;
        const CONNECTOR_SPACE: i16 = CONNECTOR_WIDTH * 150 / 100;
        const CONNECTOR_HEIGHT: i16 = CONNECTOR_WIDTH * 7 / 2;
        let is_charging = self.last_status == "Charging" || self.last_status == "Not charging";
        let full_width = (height as f64 * 1.5).round() as i16;
        let height = height as i16;
        let conn_x = x + full_width - CONNECTOR_WIDTH;
        let conn_y = (height - CONNECTOR_HEIGHT) / 2;
        let outline_width = full_width - CONNECTOR_SPACE;
        let outline_height = height * 65 / 100;
        let outline_y = (height - outline_height) / 2;
        let outline_corner = 0.277;
        let gap_width = outline_width - 2 * OUTLINE_WIDTH;
        let gap_height = outline_height - 2 * OUTLINE_WIDTH;
        let gap_x = x + OUTLINE_WIDTH;
        let gap_y = outline_y + OUTLINE_WIDTH;
        let gap_corner = outline_corner * (gap_height as f64 / outline_height as f64);
        let fill_width = (gap_width - 2 * INNER_GAP_WIDTH) * self.last_capacity as i16 / 100;
        let fill_height = gap_height - 2 * INNER_GAP_WIDTH;
        let fill_x = gap_x + INNER_GAP_WIDTH;
        let fill_y = gap_y + INNER_GAP_WIDTH;
        let fill_corner = outline_corner * (fill_height as f64 / outline_height as f64);
        // TODO: move color definitions to config
        let fill_color = if is_charging {
            Color::new_bytes(48, 209, 88, 230)
        } else if self.last_capacity <= 10 {
            Color::new_bytes(255, 69, 58, 230)
        } else {
            self.config.colors.bar_text.with_alpha(0.9)
        };
        #[rustfmt::skip]
        dc.rect((conn_x, conn_y, CONNECTOR_WIDTH as u16, CONNECTOR_HEIGHT as u16))
            .color(self.config.colors.bar_text)
            .corner_percent(0.5)
            .draw();
        dc.rect((x, outline_y, outline_width as u16, outline_height as u16))
            .color(self.config.colors.bar_text)
            .corner_percent(outline_corner)
            .draw();
        dc.rect((gap_x, gap_y, gap_width as u16, gap_height as u16))
            .color(self.config.colors.bar_background)
            .corner_percent(gap_corner)
            .draw();
        dc.rect((fill_x, fill_y, fill_width as u16, fill_height as u16))
            .color(fill_color)
            .corner_percent(fill_corner)
            .draw();
        if is_charging {
            // There doesn't seem be a way do draw a svg with outline so we
            // generate our our own bolt icon.
            let bolt_height = height as f64 * 1.1;
            let bolt_width = bolt_height * 0.583;
            let bolt_x = x as f64 + (outline_width as f64 - bolt_width) / 2.0;
            let bolt_y = (height as f64 - bolt_height) / 2.0;
            let top_x = bolt_x + bolt_width * 0.666;
            let right_inner_x = bolt_x + bolt_width * 0.55;
            let right_outer_y = bolt_y + bolt_height * 0.35;
            let right_inner_y = bolt_y + bolt_height * 0.4;
            let bot_x = bolt_x + bolt_width * 0.333;
            let left_inner_x = bolt_x + bolt_width * 0.45;
            let left_outer_y = bolt_y + bolt_height * 0.65;
            let left_inner_y = bolt_y + bolt_height * 0.6;
            let context = dc.cairo();
            context.new_path();
            context.move_to(top_x, bolt_y);
            context.line_to(right_inner_x, right_inner_y);
            context.line_to(bolt_x + bolt_width, right_outer_y);
            context.line_to(bot_x, bolt_y + bolt_height);
            context.line_to(left_inner_x, left_inner_y);
            context.line_to(bolt_x, left_outer_y);
            context.line_to(top_x, bolt_y);
            dc.set_color(self.config.colors.bar_text);
            context.fill_preserve().unwrap();
            dc.set_color(self.config.colors.bar_background);
            context.set_line_width(INNER_GAP_WIDTH as f64);
            context.set_line_cap(LineCap::Round);
            context.stroke().unwrap();
        }
        full_width as u16
    }
}

impl Widget for Battery {
    fn window(&self) -> &Window {
        &self.window
    }

    fn update(&mut self, dc: &MutexGuard<DrawingContext>, height: u16, x: i16) -> UpdateResult {
        let display = self.window.display();
        let mut capacity = std::fs::read_to_string(format!(
            "/sys/class/power_supply/{}/capacity",
            self.config.bar.power_supply
        ))
        .unwrap_or_fatal(display);
        capacity.pop();
        let mut status = std::fs::read_to_string(format!(
            "/sys/class/power_supply/{}/status",
            self.config.bar.power_supply
        ))
        .unwrap_or_fatal(display);
        status.pop();
        let n_capacity = capacity.parse().unwrap_or_fatal(display);
        if n_capacity == self.last_capacity && status == self.last_status {
            return UpdateResult::Old(self.width);
        }
        self.hover_text = format!("{}, {}", self.config.bar.power_supply, status);
        self.last_capacity = n_capacity;
        self.last_status = status;
        capacity.push('%');
        let icon_width = self.draw_icon(dc, x, height);
        let text_x = x + icon_width as i16 + 5;
        let text_width = draw_icon_and_text(
            dc,
            &capacity,
            None,
            if n_capacity <= 10 {
                Color::new_bytes(255, 69, 58, 230)
            } else {
                self.config.colors.bar_text
            },
            0,
            text_x,
            height,
        );
        self.width = (text_x - x) as u16 + text_width;
        UpdateResult::New(self.width)
    }

    fn enter(&mut self) {
        let x = self.geometry.x + self.geometry.width as i16 / 2;
        let y = self.geometry.y + self.geometry.height as i16;
        self.tooltip
            .show(&self.hover_text, ShowAt::TopCenter((x, y)));
    }

    fn leave(&mut self) {
        self.tooltip.close();
    }

    fn invalidate(&mut self) {
        self.last_capacity = u32::MAX;
    }

    fn set_geometry(&mut self, geometry: Rectangle) {
        self.geometry = geometry;
    }
}

pub struct Quit {
    window: Window,
    icon: Rc<Svg>,
    color: Color,
    redraw: bool,
}

impl Quit {
    pub fn new(bar_window: &Window, wm: &WindowManager) -> Option<Self> {
        Some(Self {
            window: create_window(bar_window, wm),
            icon: wm.resources.power().clone(),
            color: wm.config.colors.bar_text,
            redraw: true,
        })
    }
}

impl Widget for Quit {
    fn window(&self) -> &Window {
        &self.window
    }

    fn update(&mut self, dc: &MutexGuard<DrawingContext>, height: u16, x: i16) -> UpdateResult {
        if self.redraw {
            draw_icon_and_text(dc, "", Some(self.icon.as_ref()), self.color, 0, x, height);
            self.redraw = false;
            UpdateResult::New(height)
        } else {
            UpdateResult::Old(height)
        }
    }

    fn click(&mut self, _: &ButtonPressEvent) {
        quit_dialog();
    }

    fn invalidate(&mut self) {
        self.redraw = true;
    }
}
