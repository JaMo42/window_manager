use crate::{
    config::Size,
    draw::{Alignment, DrawingContext, Svg},
    event::{is_button_press, x_event_source, EventSink, Signal},
    ewmh::{self, WindowType},
    monitors::monitors,
    mouse::{BUTTON_1, BUTTON_3},
    mouse_block::MouseBlock,
    rectangle::{Rectangle, ShowAt},
    window_manager::{WindowKind, WindowManager},
    x::{InputOnlyWindow, Window, XcbWindow},
};
use std::{rc::Rc, sync::Arc};
use xcb::{
    x::{ButtonPressEvent, EventMask, KeyPressEvent},
    Event,
};

/// Special icons
pub enum Indicator {
    Check,
    Diamond,
    Circle,
    Exclamation,
}

impl Indicator {
    fn symbol(&self) -> &str {
        match *self {
            Self::Check => "✔",
            Self::Diamond => "♦",
            Self::Circle => "⚫",
            Self::Exclamation => "❗",
        }
    }
}

pub struct Action<T> {
    name: String,
    info: String,
    icon: Option<Rc<Svg>>,
    indicator: Option<Indicator>,
    action: Box<dyn FnMut(T)>,
    is_hovered: bool,
}

impl<T> Action<T> {
    fn new(name: String, action: Box<dyn FnMut(T)>) -> Self {
        Self {
            name,
            info: String::new(),
            icon: None,
            indicator: None,
            action,
            is_hovered: false,
        }
    }

    pub fn icon(&mut self, icon: Option<Rc<Svg>>) -> &mut Self {
        self.icon = icon;
        self
    }

    pub fn indicator(&mut self, indicator: Option<Indicator>) -> &mut Self {
        self.indicator = indicator;
        self
    }

    pub fn info(&mut self, info: String) -> &mut Self {
        self.info = info;
        self
    }
}

enum Item<T> {
    Action(Action<T>),
    Divider,
}

impl<T> Item<T> {
    fn action(&self) -> &Action<T> {
        if let Self::Action(action) = self {
            action
        } else {
            panic!("item is not an action")
        }
    }

    fn action_mut(&mut self) -> &mut Action<T> {
        if let Self::Action(action) = self {
            action
        } else {
            panic!("item is not an action")
        }
    }

    fn is_action(&self) -> bool {
        matches!(self, Self::Action(_))
    }
}

#[derive(Default, Debug)]
pub struct Layout {
    padding: i16,
    space: i16,
    action_height: u16,
    divider: Rectangle,
    divider_space: i16,
    item_spacing: i16,
    content_x: i16,
    indicator_width: u16,
    action_width: u16,
    highlight: Rectangle,
}

impl Layout {
    fn compute<T>(menu: &ContextMenu<T>, dpmm: f64) -> Self {
        // Should maybe depend on font size
        let padding = Size::Physical(2.0).resolve(Some(dpmm), None, None) as i16;
        let space = Size::Physical(1.0).resolve(Some(dpmm), None, None) as i16;
        let dc = menu.wm.drawing_context.lock();
        dc.set_font(&menu.wm.config.bar.font);
        let action_height = dc.font_height(None);
        let mut action_width = 0;
        for action in menu.items.iter().filter(|a| matches!(a, Item::Action(_))) {
            let action = action.action();
            let mut my_width = 0;
            if action.icon.is_some() {
                my_width += action_height + space as u16;
            }
            my_width += dc.text_width(&action.name, None);
            if !action.info.is_empty() {
                my_width += dc.text_width(&action.info, None);
            }
            action_width = action_width.max(my_width);
        }
        let fullwidth_character_width = dc.fullwidth_character_width(None) as i16;
        drop(dc);
        let divider_height = Size::Physical(0.3).resolve(Some(dpmm), None, None);
        let content_x;
        let divider;
        if menu.has_indicator {
            action_width += padding as u16;
            action_width += fullwidth_character_width as u16;
            content_x = padding + fullwidth_character_width;
            divider = Rectangle::new(
                2 * padding,
                0,
                action_width - 2 * padding as u16,
                divider_height,
            );
        } else {
            content_x = padding;
            divider = Rectangle::new(padding, 0, action_width, divider_height);
        }
        let item_spacing = Size::Physical(1.0).resolve(Some(dpmm), None, None) as i16;
        let highlight_grow = (action_height as f64 * 0.15).round() as u16;
        let highlight = Rectangle::new(
            padding / 2,
            -(highlight_grow as i16 / 2),
            action_width + padding as u16,
            action_height + highlight_grow,
        );
        let divider_space = item_spacing / 2;
        Self {
            padding,
            space,
            action_height,
            divider,
            divider_space,
            item_spacing,
            content_x,
            indicator_width: fullwidth_character_width as u16,
            action_width,
            highlight,
        }
    }

    fn icon(&self) -> Rectangle {
        Rectangle::new(self.content_x, 0, self.action_height, self.action_height)
    }

    fn indicator(&self) -> Rectangle {
        Rectangle::new(self.padding, 0, self.indicator_width, self.action_height)
    }

    fn divider(&self) -> Rectangle {
        self.divider
    }

    fn item(&self) -> Rectangle {
        Rectangle::new(self.padding, 0, self.action_width, self.action_height)
    }

    fn text_x(&self, with_icon: bool) -> i16 {
        if with_icon {
            self.content_x + self.action_height as i16 + self.space
        } else {
            self.content_x
        }
    }

    fn highlight(&self, base_y: i16) -> Rectangle {
        self.highlight.with_y(self.highlight.y + base_y)
    }
}

pub struct ContextMenu<T> {
    wm: Arc<WindowManager>,
    items: Vec<Item<T>>,
    action_windows: Vec<InputOnlyWindow>,
    window: Window,
    has_indicator: bool,
    callback_data: T,
    after: Option<Box<dyn FnMut(T)>>,
    geometry: Rectangle,
    layout: Layout,
    mouse_block: Option<MouseBlock>,
    selected: Option<usize>,
}

impl<T: Clone> ContextMenu<T> {
    pub fn new(wm: Arc<WindowManager>, callback_data: T) -> Self {
        let visual = wm.display.truecolor_visual();
        let window = Window::builder(wm.display.clone())
            .visual(visual.id)
            .depth(visual.depth)
            .attributes(|attributes| {
                attributes
                    .border_pixel(0)
                    .background_pixel(0)
                    .event_mask(EventMask::KEY_PRESS)
                    .colormap(visual.colormap);
            })
            .build();
        ewmh::set_window_type(&window, WindowType::PopupMenu);
        wm.set_window_kind(&window, WindowKind::ContextMenu);
        Self {
            wm,
            items: Vec::new(),
            action_windows: Vec::new(),
            window,
            has_indicator: false,
            callback_data,
            after: None,
            geometry: Rectangle::zeroed(),
            layout: Layout::default(),
            mouse_block: None,
            selected: None,
        }
    }

    fn destroy(&self) {
        self.wm.display.ungrab_keyboard();
        for i in self.action_windows.iter() {
            if !i.is_none() {
                self.wm.remove_all_contexts(&i.handle());
                i.destroy(&self.wm.display);
            }
        }
        if let Some(mouse_block) = &self.mouse_block {
            mouse_block.destroy(&self.wm);
        }
        self.wm.remove_all_contexts(&self.window);
        self.window.destroy();
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn add_action(&mut self, name: impl ToString, action: Box<dyn FnMut(T)>) -> &mut Action<T> {
        self.items
            .push(Item::Action(Action::new(name.to_string(), action)));
        self.items.last_mut().unwrap().action_mut()
    }

    pub fn add_divider(&mut self) {
        self.items.push(Item::Divider);
    }

    pub fn after(&mut self, callback: Box<dyn FnMut(T)>) {
        self.after = Some(callback);
    }

    pub fn show_at(&mut self, at: ShowAt) {
        if !self.has_indicator {
            for i in self.items.iter() {
                if let Item::Action(action) = i {
                    if action.indicator.is_some() {
                        self.has_indicator = true;
                        break;
                    }
                }
            }
        }
        self.action_windows.reserve_exact(self.items.len());
        for i in self.items.iter() {
            match i {
                Item::Action(_) => {
                    let window = InputOnlyWindow::builder()
                        .with_mouse(true, false, false)
                        .with_crossing()
                        .with_parent(self.window.handle())
                        .build(&self.wm.display);
                    self.wm
                        .set_window_kind(&window, WindowKind::ContextMenuItem);
                    self.action_windows.push(window);
                }
                Item::Divider => self.action_windows.push(InputOnlyWindow::new_none()),
            }
        }
        let dpmm = monitors().at(at.anchor()).dpmm();
        self.layout = Layout::compute(self, dpmm);
        let dc = self.wm.drawing_context.lock();
        dc.rect(monitors().primary().geometry().at(0, 0))
            .color(self.wm.config.colors.bar_background)
            .draw();
        let (width, height) = self.paint(&dc);
        self.geometry = at.translate((0, 0, width, height));
        self.window.move_and_resize(self.geometry);
        let mut y = self.layout.padding - self.layout.item_spacing;
        for (idx, item) in self.items.iter().enumerate() {
            y += self.layout.item_spacing;
            // TODO: also create the input windows in this loop
            match item {
                Item::Action(_) => {
                    self.action_windows[idx]
                        .move_and_resize(&self.wm.display, self.layout.item().with_y(y));
                    y += self.layout.action_height as i16;
                }
                Item::Divider => {
                    y += self.layout.divider_space;
                }
            }
        }
        self.window.map();
        self.window.map_subwindows();
        dc.render(&self.window, self.geometry.at(0, 0));
        drop(dc);
        self.mouse_block = Some(MouseBlock::new_invisible(
            &self.wm,
            monitors().at(at.anchor()),
        ));
        self.window.raise();
        self.wm.display.grab_keyboard(self.window.handle());
    }

    fn paint(&self, dc: &DrawingContext) -> (u16, u16) {
        let fg = self.wm.config.colors.context_menu_text;
        let bg = self.wm.config.colors.context_menu_background;
        let mut y = self.layout.padding - self.layout.item_spacing;
        dc.rect(self.geometry.at(0, 0)).color(bg).draw();
        dc.set_font(&self.wm.config.bar.font);
        dc.set_color(fg);
        for item in self.items.iter() {
            y += self.layout.item_spacing;
            if matches!(item, Item::Divider) {
                y += self.layout.divider_space;
                let mut rect = self.layout.divider().with_y(y);
                rect.y -= rect.height as i16;
                dc.rect(rect)
                    .color(self.wm.config.colors.context_menu_divider)
                    .corner_percent(0.5)
                    .draw();
                dc.set_color(fg);
                continue;
            }
            let action = item.action();
            if action.is_hovered {
                dc.rect(self.layout.highlight(y)).corner_percent(0.2).draw();
                dc.set_color(bg.with_alpha(1.0));
            } else {
                dc.set_color(fg);
            }
            if let Some(indicator) = &action.indicator {
                dc.text(indicator.symbol(), self.layout.indicator().with_y(y))
                    .vertical_alignment(Alignment::CENTER)
                    .horizontal_alignment(Alignment::CENTER)
                    .draw();
            }
            let mut text_x;
            if let Some(icon) = &action.icon {
                dc.draw_svg(icon, self.layout.icon().with_y(y));
                text_x = self.layout.text_x(true);
            } else {
                text_x = self.layout.text_x(false);
            }
            text_x += dc
                .text(&action.name, (text_x, y, 0, self.layout.action_height))
                .vertical_alignment(Alignment::CENTER)
                .draw()
                .width as i16;
            if !action.info.is_empty() {
                dc.text(&action.info, (text_x, y, 0, self.layout.action_height))
                    .color(fg.scale(0.6).with_alpha(1.0))
                    .vertical_alignment(Alignment::CENTER)
                    .draw();
                dc.set_color(fg);
            }
            y += self.layout.action_height as i16;
        }
        (
            self.layout.action_width + 2 * self.layout.padding as u16,
            (y + self.layout.padding) as u16,
        )
    }

    fn update(&self) {
        let dc = self.wm.drawing_context.lock();
        self.paint(&dc);
        dc.render(&self.window, self.geometry.at(0, 0));
    }

    fn get_event_action_index(&self, event: XcbWindow) -> usize {
        self.action_windows
            .iter()
            .position(|w| *w == event)
            .unwrap()
    }

    fn get_event_action(&mut self, event: XcbWindow) -> &mut Action<T> {
        let idx = self.get_event_action_index(event);
        self.items[idx].action_mut()
    }

    fn finish(&mut self, item: XcbWindow) {
        let data = self.callback_data.clone();
        if let Some(mut after) = self.after.take() {
            let action = self.get_event_action(item);
            (action.action)(data.clone());
            after(data);
        } else {
            let action = self.get_event_action(item);
            (action.action)(data);
        }
        self.destroy();
        self.wm.remove_event_sink(self.id());
    }

    fn cancel(&mut self, deferr_removal: bool) {
        if let Some(mut after) = self.after.take() {
            after(self.callback_data.clone());
        }
        self.destroy();
        if deferr_removal {
            self.wm.signal_remove_event_sink(self.id());
        } else {
            self.wm.remove_event_sink(self.id());
        }
    }

    fn move_selected(&mut self, d: isize) {
        let mut sel = self
            .selected
            .unwrap_or(if d < 0 { 0 } else { self.items.len() - 1 }) as isize;
        loop {
            sel += d;
            if sel < 0 {
                sel = self.items.len() as isize - 1;
            } else if sel == self.items.len() as isize {
                sel = 0;
            }
            if self.items[sel as usize].is_action() {
                break;
            }
        }
        if let Some(before) = self.selected {
            self.items[before].action_mut().is_hovered = false;
        }
        self.selected = Some(sel as usize);
        self.items[sel as usize].action_mut().is_hovered = true;
        self.update();
    }

    fn key_press(&mut self, event: &KeyPressEvent) {
        use x11::keysym::*;
        let sym = self.wm.display.keycode_to_keysym(event.detail());
        // FIXME: up/down logic assumes that neither the first nor the last item
        // is a divider.
        #[allow(non_upper_case_globals)]
        match sym as u32 {
            XK_Up => self.move_selected(-1),
            XK_Down => self.move_selected(1),
            XK_space | XK_Return => {
                if let Some(selected) = self.selected {
                    self.finish(self.action_windows[selected].handle());
                }
            }
            XK_Escape => self.cancel(false),
            _ => {}
        }
    }

    fn menu_event(&mut self, event: &Event) {
        let event = if let Event::X(event) = event {
            event
        } else {
            return;
        };
        use xcb::x::Event::*;
        if let KeyPress(e) = event {
            self.key_press(e)
        }
    }

    fn item_event(&mut self, event: &Event, source: XcbWindow) {
        let event = if let Event::X(event) = event {
            event
        } else {
            return;
        };
        use xcb::x::Event::*;
        match event {
            EnterNotify(_) => {
                if let Some(selected) = self.selected {
                    self.items[selected].action_mut().is_hovered = false;
                }
                let idx = self.get_event_action_index(source);
                self.selected = Some(idx);
                self.items[idx].action_mut().is_hovered = true;
                self.update();
            }
            LeaveNotify(_) => {
                self.selected = None;
                let action = self.get_event_action(source);
                action.is_hovered = false;
                self.update();
            }
            ButtonPress(e) => {
                if e.detail() == BUTTON_1 || e.detail() == BUTTON_3 {
                    self.finish(source);
                }
            }
            _ => {}
        }
    }
}

impl<T: Clone> EventSink for ContextMenu<T> {
    fn accept(&mut self, event: &Event) -> bool {
        let source = if let Some(source) = x_event_source(event) {
            source
        } else {
            return false;
        };
        match self.wm.get_window_kind(&source) {
            WindowKind::ContextMenu => {
                self.menu_event(event);
            }
            WindowKind::ContextMenuItem => {
                self.item_event(event, source);
            }
            // Checking if it's our mouse block should not be necessary as this
            // if only a temporary sink and even if there are somehow multiple
            // blocks ours should always be the topmost.
            WindowKind::MouseBlock /*if self.mouse_block.as_ref().unwrap().eq(&source)*/ => {
                self.cancel(false);
            }
            _ if is_button_press(event) => {
                // We want to propagate the event but since `cancel` removes
                // our event sink we should stop iterating over the sink list.
                use xcb::x::Event::ButtonPress;
                let inner = match event {
                    Event::X(ButtonPress(inner)) => inner,
                    _ => unreachable!()
                };
                let copy = ButtonPressEvent::new(
                    inner.detail(),
                    inner.time(),
                    inner.root(),
                    inner.event(),
                    inner.child(),
                    inner.root_x(),
                    inner.root_y(),
                    inner.event_x(),
                    inner.event_y(),
                    inner.state(),
                    inner.same_screen(),
                );
                self.wm.display.put_back_event(Ok(Event::X(ButtonPress(copy))));
                self.cancel(false);
            }
            _ => return false,
        }
        true
    }

    fn signal(&mut self, signal: &Signal) {
        match signal {
            Signal::Quit => self.destroy(),
            _ => {
                if !matches!(signal, Signal::UpdateBar(_)) {
                    self.cancel(true);
                }
            }
        }
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[
            ButtonPressEvent::NUMBER,
            KeyPressEvent::NUMBER,
            EnterNotifyEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
        ]
    }
}
