use crate::{
    action,
    client::Client,
    color::{BorderColor, Color},
    color_scheme::ColorScheme,
    draw::{BuiltinResources, ColorKind, DrawingContext, GradientSpec, Svg},
    error::fatal_error,
    layout::ButtonLayout,
    rectangle::Rectangle,
    window_manager::{WindowKind, WindowManager},
    x::{Window, XcbWindow},
};
use parking_lot::MutexGuard;
use std::{rc::Rc, sync::Arc};
use xcb::{x::EventMask, Xid};

/// Values that differ between buttons on the same client.
struct Props {
    icon: Rc<Svg>,
    normal_color: Color,
    hovered_color: Color,
    action: fn(&Client),
}

impl Props {
    fn new(icon: Rc<Svg>, normal_color: Color, hovered_color: Color, action: fn(&Client)) -> Self {
        Self {
            icon,
            normal_color,
            hovered_color,
            action,
        }
    }

    fn get_named(name: &str, icons: &BuiltinResources, colors: &ColorScheme) -> Option<Self> {
        Some(match name {
            "close" => Self::new(
                icons.close_button().clone(),
                colors.close_button,
                colors.close_button_hovered,
                action::close_client,
            ),
            "maximize" => Self::new(
                icons.maximize_button().clone(),
                colors.maximize_button,
                colors.maximize_button_hovered,
                action::toggle_maximized,
            ),
            "minimize" => Self::new(
                icons.minimize_button().clone(),
                colors.minimize_button,
                colors.minimize_button_hovered,
                action::minimize,
            ),
            _ => None?,
        })
    }
}

pub struct Button {
    layout: ButtonLayout,
    normal_color: Color,
    hovered_color: Color,
    icon: Rc<Svg>,
    window: Window,
    geometry: Rectangle,
    action: fn(&Client),
    is_circle: bool,
}

impl Button {
    pub fn from_string(
        wm: &WindowManager,
        client: &Arc<Client>,
        layout: ButtonLayout,
        name: &str,
    ) -> Self {
        let props = match Props::get_named(name, &wm.resources, &wm.config.colors) {
            Some(props) => props,
            None => fatal_error(&wm.display, format!("invalid button name: {name}")),
        };
        Self::new(
            wm,
            client,
            layout,
            props.icon,
            props.normal_color,
            props.hovered_color,
            props.action,
        )
    }

    fn new(
        wm: &WindowManager,
        client: &Arc<Client>,
        layout: ButtonLayout,
        icon: Rc<Svg>,
        normal_color: Color,
        hovered_color: Color,
        action: fn(&Client),
    ) -> Self {
        let display = wm.display.clone();
        let visual = *display.truecolor_visual();
        let window = Window::builder(display)
            .parent(client.frame().handle())
            .size(layout.size(), layout.size())
            .depth(visual.depth)
            .visual(visual.id)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .event_mask(
                        EventMask::BUTTON_PRESS
                            | EventMask::BUTTON_RELEASE
                            | EventMask::ENTER_WINDOW
                            | EventMask::LEAVE_WINDOW,
                    )
                    .colormap(visual.colormap)
                    .background_pixel(0)
                    .border_pixel(0);
            })
            .build();
        wm.associate_client(&window, client);
        wm.set_window_kind(&window, WindowKind::FrameButton);
        let geometry = Rectangle::new(0, 0, layout.size(), layout.size());
        Self {
            layout,
            normal_color,
            hovered_color,
            icon,
            window,
            geometry,
            action,
            is_circle: wm.config.window.circle_buttons,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn set_geometry(&mut self, geometry: Rectangle) {
        self.geometry = geometry;
        self.window.move_and_resize(geometry);
    }

    pub fn set_layout(&mut self, layout: ButtonLayout) {
        self.layout = layout;
    }

    pub fn draw(
        &self,
        dc: &MutexGuard<DrawingContext>,
        border_color: &BorderColor,
        is_hovered: bool,
    ) {
        let color = if self.is_circle {
            self.hovered_color.scale(0.3)
        } else if is_hovered {
            self.hovered_color
        } else {
            self.normal_color
        };
        dc.rect((0, 0, self.layout.size(), self.layout.size()))
            .gradient(GradientSpec::new_vertical(
                border_color.top(),
                border_color.border(),
            ))
            .draw();
        if self.is_circle {
            let color = if is_hovered || border_color.is_focused() {
                self.hovered_color
            } else {
                self.normal_color
            };
            let outline_color = color.scale(0.9);
            dc.ellipse(*self.layout.circle_rect())
                .color(color)
                .stroke(1, ColorKind::Solid(outline_color))
                .draw();
        }
        if !self.is_circle || is_hovered {
            dc.draw_colored_svg(&self.icon, color, *self.layout.icon_rect());
        }
        dc.render(&self.window, (0, 0, self.layout.size(), self.layout.size()));
    }

    pub fn click(&self, client: &Client) {
        (self.action)(client);
    }
}

impl PartialEq<XcbWindow> for &Button {
    fn eq(&self, other: &XcbWindow) -> bool {
        self.window.handle() == *other
    }
}

impl std::fmt::Display for Button {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Button({})", self.window.handle().resource_id())
    }
}
