use crate::{
    color::Color,
    draw::{Alignment, DrawingContext},
    markup::remove_markup,
    process::run,
    rectangle::Rectangle,
    x::{randr::main_monitor_geometry, Display, Window},
};
use pango::FontDescription;
use std::sync::Arc;
use xcb::{x::EventMask, Event};

/// Shows the given markup message but resumes the program.
pub fn display_fatal_error(display: &Display, message: String) {
    use xcb::x::Event::*;
    log::error!(
        "Error: {}",
        remove_markup(&message).replace('\n', "\n     | ")
    );
    // We need an arc for this but in a lot of places where we call this
    // function we don't have access to one.
    // `Arc::from_raw` *should* only be called with a pointer obtained from
    // `Arc::into_raw` but since we always get the display reference from an
    // `Arc` this should be fine as well especially since we quit shortly after
    // calling this anyways and don't need to worry about consequences later on.
    let display = unsafe { Arc::from_raw(display as *const Display) };
    let font = FontDescription::from_string("sans 24");
    let background_color = Color::new_rgb(0.12, 0.12, 0.12);
    let text_color = Color::new_rgb(0.91, 0.92, 0.92);
    let border = 50;
    let geometry: Rectangle = main_monitor_geometry(&display).into();
    let draw = DrawingContext::create(display.clone(), (geometry.width, geometry.height));
    let visual = display.truecolor_visual();
    let window = Window::builder(display.clone())
        .geometry(geometry)
        .visual_info(visual)
        .attributes(|attributes| {
            attributes
                .override_redirect()
                .background_pixel(0)
                .border_pixel(0)
                .event_mask(EventMask::KEY_PRESS | EventMask::BUTTON_PRESS);
        })
        .build();
    window.map();
    window.raise();

    // Always draw to the window at (0, 0)
    let geometry = geometry.at(0, 0);
    let content_area = *geometry.clone().resize(-border);
    draw.fill_rect(geometry, background_color);
    draw.set_font(&font);
    draw.set_color(text_color);
    draw.markup(&message, content_area).draw();
    draw.set_color(text_color);
    draw.text("Press any key to quit", content_area)
        .color(text_color)
        .vertical_alignment(Alignment::BOTTOM)
        .draw();
    draw.render(&window, geometry);

    display.set_input_focus(window.handle());
    loop {
        if let Ok(Event::X(KeyPress(_) | ButtonPress(_))) = display.next_event() {
            break;
        }
    }
}

/// Shows the given markup message and aborts the program after any key is pressed.
pub fn fatal_error(display: &Display, message: String) -> ! {
    display_fatal_error(display, message);
    std::process::exit(1);
}

pub trait OrFatal<T> {
    fn or_fatal(self, display: &Display);

    fn unwrap_or_fatal(self, display: &Display) -> T;
}

impl<T, E> OrFatal<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn or_fatal(self, display: &Display) {
        if let Err(error) = self {
            fatal_error(display, format!("{error:#}"))
        }
    }

    fn unwrap_or_fatal(self, display: &Display) -> T {
        match self {
            Ok(value) => value,
            Err(error) => fatal_error(display, format!("{error:#}")),
        }
    }
}

pub trait LogError<T> {
    fn log_error(self) -> Option<T>;
}

impl<T, E: std::fmt::Display> LogError<T> for Result<T, E> {
    /// Turn a result into an option, logging the error if it was one.
    fn log_error(self) -> Option<T> {
        match self {
            Ok(x) => Some(x),
            Err(e) => {
                log::error!("{e}");
                None
            }
        }
    }
}

pub trait LogNone<T> {
    fn log_none(self, msg: &str) -> Option<T>;
}

impl<T> LogNone<T> for Option<T> {
    /// Logs the given error message if the option is `None`.
    fn log_none(self, msg: &str) -> Self {
        if self.is_none() {
            log::error!("{msg}");
        }
        self
    }
}

pub fn message_box(title: &str, body: &str) {
    run(&[
        "window_manager_message_box",
        title,
        body,
        "--kind",
        "Error",
        "--font-size",
        "20",
    ])
    .ok();
}
