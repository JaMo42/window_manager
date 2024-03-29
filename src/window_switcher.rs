use crate::{
    action::close_client,
    client::Client,
    config::{Config, Size},
    draw::{create_xcb_surface, Alignment, DrawingContext, GradientSpec},
    event::{x_event_source, EventSink, Signal, SinkStorage},
    monitors::monitors,
    rectangle::Rectangle,
    window_manager::{WindowKind, WindowManager},
    x::{Display, InputOnlyWindow, Window, XcbWindow},
};
use cairo::XCBSurface;
use pango::{EllipsizeMode, FontDescription};
use std::sync::Arc;
use x11::keysym::XK_Alt_L;
use xcb::{
    x::{EnterNotifyEvent, EventMask, KeyButMask, KeyPressEvent, KeyReleaseEvent},
    Event, Xid,
};

// Detailed layout system explanation
//
// Functions:
//   - WindowSwitcher::layout:
//     Goes through all available layouts and get the largest we can use,
//     preferrably with previews using the `WindowSwitcher::try_layout`. method
//     After this it creates the `WindowSwitcherClients` structures with their
//     input windows and sets all their positions as wellas the container window
//     geometry and maps all the input windows.  If this function returns false
//     we were not able  to create a window switcher with any layout. The
//     `window_switcher` function will abort in this case.
//
//   - WindowSwitcher::try_layout:
//     Attempts to create a layout with a specific layout size.  This first
//     determines the sizes of ceach client using the `Layout::client` method
//     and then attempts to build the grid with those sizes using the
//     `WindowSwitcher::layout_grid` mehod.
//
//   - Layout::client
//     This only sets the sizes of a `ClientLayout` but leaves the positions
//     unset as we need to know the sizes to determine the positions.
//
//   - WindowSwitcher::layout_grid
//     Tries to get the grid layout and container size, this is also where we
//     check if the layout cannot be used at all.  To get the grid we first build
//     a list of row counts and how many leftover spaces that row count has and
//     sort that list minimize the leftover spaces while minimizing the row count.
//     This is definetly not an optimal way of minimizing free space as it ignores
//     the actual sizes of the clients but it's east to implement.  We then go
//     throught that list from best to worst and pick the first option that works
//     using the `try_distribute` function.  If no option works we disable
//     previews and try again.  If it still doesn't work we reject the layout
//     completely.  If the layout does work we then go through the generated
//     grid and set the client positions accordingly using
//     `ClientLayout::set_position`.
//
//   - `try_distribute`
//     Takes the list of row-count options from `WindowSwitcher::layout_grid`
//     and tries to build the grid with each row count using the
//     `try_distribute_with_row_count` function.
//
//   - `try_distribute_with_row_count`
//     First determines the column count based on the given row count and the
//     Builds the grid by growing the last row and adding a new row if that row
//     reached the column count.  If a row grows wider than the maxium width
//     allowed by the layout the row-count option is rejected and we go back to
//     `try_distrbute` to try the next option.  If after finishing building the
//     grid it's height is taller than the maximum height allowed by the layout
//     the row-count is rejected as well.  Note that this function only creates
//     a 2d vector with the indices of clients that should be in that position
//     in the grid but does not set any positions.
//
//   - `ClientLayout::set_position`
//     Receives the top-left corner of the client position including padding
//     and sets all its positions accordingly using the sizes that were already
//     computed by `Layout::client`.

#[derive(Debug)]
struct Layout {
    padding: i16,
    spacing: i16,
    client_padding: i16,
    title_height: u16,
    preview_height: u16,
    min_preview_width: u16,
    max_preview_width: u16,
    max_width: u16,
    max_height: u16,
}

impl Layout {
    fn compute(_config: &Config, font_height: u16, preview_height_percent: u32) -> Self {
        let dpmm = monitors().primary().dpmm();
        let screen_size = *monitors().primary().geometry();
        let padding = Size::Physical(2.0).resolve(Some(dpmm), None, None) as i16;
        let spacing = padding / 2;
        let client_padding = spacing;
        let title_height = Size::PercentOfFont(1.2).resolve(None, None, Some(font_height));
        let preview_height = (screen_size.height as u32 * preview_height_percent / 100) as u16;
        let min_preview_width = preview_height / 2;
        let max_preview_width = preview_height * 2;
        let max_width = (screen_size.width as u32 * 90 / 100) as u16;
        let max_height = (screen_size.height as u32 * 90 / 100) as u16;
        Self {
            padding,
            spacing,
            client_padding,
            title_height,
            preview_height,
            min_preview_width,
            max_preview_width,
            max_width,
            max_height,
        }
    }

    fn client(&self, aspect_ratio: f64, has_icon: bool) -> ClientLayout {
        let mut height = self.preview_height;
        let tentative_width = (self.preview_height as f64 * aspect_ratio).round() as u16;
        let mut width = tentative_width.clamp(self.min_preview_width, self.max_preview_width);
        if width != tentative_width {
            height = (width as f64 / aspect_ratio).round() as u16;
        }
        if height > self.preview_height {
            height = self.preview_height;
            width = (self.preview_height as f64 * aspect_ratio).round() as u16;
            width = width.clamp(self.min_preview_width, self.max_preview_width);
        }
        let rect = Rectangle::new(
            0,
            0,
            width + 2 * self.client_padding as u16,
            height + 2 * self.client_padding as u16 + self.title_height,
        );
        let title_width_offset = self.title_height + if has_icon { self.title_height } else { 0 };
        ClientLayout {
            background: rect,
            icon: Rectangle::new(0, 0, self.title_height, self.title_height),
            title: Rectangle::new(0, 0, width - title_width_offset, self.title_height),
            close_button: Rectangle::new(0, 0, self.title_height, self.title_height),
            preview: Rectangle::new(0, 0, width, height),
        }
    }
}

#[derive(Debug)]
struct ClientLayout {
    background: Rectangle,
    icon: Rectangle,
    title: Rectangle,
    close_button: Rectangle,
    preview: Rectangle,
}

impl ClientLayout {
    fn width(&self) -> u16 {
        self.background.width
    }

    fn height(&self) -> u16 {
        self.background.height
    }

    fn set_position(&mut self, x: i16, y: i16, layout: &Layout, has_icon: bool) {
        let top = y + layout.client_padding;
        let left = x + layout.client_padding;
        self.background = self.background.at(x, y);
        self.icon = self.icon.at(left, top);
        let title_x = if has_icon {
            left + self.icon.width as i16
        } else {
            left
        };
        self.title = self.title.at(title_x, top);
        self.close_button = self
            .close_button
            .at(
                // We add the initial x here for `disable_preview` to work before
                // this function is called.
                self.close_button.x + left + (self.preview.width - self.close_button.width) as i16,
                top,
            )
            .scale(80);
        self.preview = self.preview.at(left, top + self.icon.height as i16);
    }

    fn layout_input_windows(&self, display: &Display, client: &WindowSwitcherClient) {
        client
            .input_window
            .move_and_resize(display, self.background);
        let mut close_button = self.close_button;
        close_button.x -= self.background.x;
        close_button.y -= self.background.y;
        client.close_button.move_and_resize(display, close_button);
    }

    fn disable_preview(&mut self, width: u16) {
        self.background.height -= self.preview.height;
        // the width passed will be the maximum preview width so this never overflows
        let width_delta = width - self.preview.width;
        self.background.width = width;
        self.title.width += width_delta;
        self.close_button.x += width_delta as i16;
    }
}

struct WindowSwitcherClient {
    client: Arc<Client>,
    layout: ClientLayout,
    input_window: InputOnlyWindow,
    close_button: InputOnlyWindow,
    hovered: bool,
    close_button_hovered: bool,
    close_button_pressed: bool,
    selected: bool,
    surface: XCBSurface,
}

fn try_distribute_with_row_count(
    clients: &[ClientLayout],
    layout: &Layout,
    rows: &mut Vec<Vec<usize>>,
    row_count: usize,
) -> (bool, u16) {
    rows.clear();
    let max_columns = (clients.len() + row_count - 1) / row_count;
    let row_width_0 = 2 * layout.padding as u16;
    let mut row_width = row_width_0;
    let mut max_row_width = 0;
    rows.push(Vec::new());
    let mut current_row = unsafe { rows.last_mut().unwrap_unchecked() };
    for (index, client) in clients.iter().enumerate() {
        if current_row.len() == max_columns {
            rows.push(Vec::new());
            current_row = unsafe { rows.last_mut().unwrap_unchecked() };
            max_row_width = max_row_width.max(row_width - row_width_0);
            row_width = row_width_0;
        } else if row_width + client.width() > layout.max_width {
            return (false, 0);
        }
        current_row.push(index);
        if row_width != row_width_0 {
            row_width += layout.spacing as u16;
        }
        row_width += client.width();
    }
    let mut height = 2 * layout.padding as u16;
    for row in rows {
        let row_height = row
            .iter()
            .map(|&index| clients[index].height())
            .max()
            .unwrap();
        height += row_height + layout.spacing as u16 + 1;
    }
    height -= layout.spacing as u16;
    if height > layout.max_height {
        return (false, 0);
    }
    max_row_width = max_row_width.max(row_width - row_width_0);
    (true, max_row_width)
}

fn try_distribute(
    clients: &[ClientLayout],
    layout: &Layout,
    rows: &mut Vec<Vec<usize>>,
    options: &[(usize, usize)],
) -> Option<u16> {
    for &(_, row_count) in options {
        let (ok, max_row_width) = try_distribute_with_row_count(clients, layout, rows, row_count);
        if ok {
            return Some(max_row_width);
        }
    }
    None
}

struct WindowSwitcher {
    wm: Arc<WindowManager>,
    layouts: Vec<Layout>,
    used_layout: usize,
    window: Window,
    geometry: Rectangle,
    font: FontDescription,
    clients: Vec<WindowSwitcherClient>,
    hovered: usize,
    switch_index: usize,
    shift: KeyButMask,
    // we need a different deletion method when handling a signal but I don't
    // want to pass around some flag so we store this flag and set if from the
    // signal handler.
    in_signal_handler: bool,
    previews: bool,
    /// Indices of frist item in each row.
    row_starts: Vec<usize>,
}

impl WindowSwitcher {
    fn new(wm: Arc<WindowManager>) -> Self {
        let visual = wm.display.truecolor_visual();
        let window = Window::builder(wm.display.clone())
            .visual(visual.id)
            .depth(visual.depth)
            .geometry((0, 0, 10, 10)) // we have no meaningful geometry yet but
            // we want to be sure we have the same size as the surface (not sure
            // if this is actually neccessary)
            .attributes(|attributes| {
                attributes
                    .override_redirect()
                    .background_pixel(0)
                    .border_pixel(0)
                    .colormap(visual.colormap)
                    .event_mask(
                        EventMask::KEY_PRESS
                            | EventMask::KEY_RELEASE
                            | EventMask::ENTER_WINDOW
                            | EventMask::LEAVE_WINDOW,
                    );
            })
            .build();
        log::trace!("window switcher: window: {window}");
        wm.set_window_kind(&window, WindowKind::WindowSwitcher);
        let font = wm
            .config
            .client_layout()
            .borrow()
            .get(monitors().primary())
            .title_font()
            .clone();
        let font_height = wm.drawing_context.lock().font_height(Some(&font));
        let layouts = wm
            .config
            .general
            .window_switcher_sizes
            .iter()
            .map(|&preview_height_percent| {
                Layout::compute(&wm.config, font_height, preview_height_percent)
            })
            .collect();
        let shift = KeyButMask::from_bits_truncate(wm.modmap.borrow().shift().bits());
        Self {
            wm,
            layouts,
            used_layout: 0,
            window,
            geometry: Rectangle::zeroed(),
            font,
            clients: Vec::new(),
            hovered: usize::MAX,
            switch_index: 1,
            shift,
            in_signal_handler: false,
            previews: true,
            row_starts: Vec::new(),
        }
    }

    fn clear_clients(&mut self) {
        for client in self.clients.drain(..) {
            self.wm.remove_all_contexts(&client.close_button);
            self.wm.remove_all_contexts(&client.input_window);
            client.input_window.destroy(&self.wm.display);
            // close button is detroyed as subwindow
        }
        self.row_starts.clear();
    }

    fn destroy(&mut self) {
        self.clear_clients();
        self.window.destroy();
        self.wm.remove_all_contexts(&self.window);
        self.wm.active_workspace().no_focus = false;
        if self.in_signal_handler {
            self.wm.signal_remove_event_sink(self.id());
        } else {
            self.wm.remove_event_sink(self.id());
        }
    }

    /// Figures out how the rows are arranged and positions the clients.
    /// Returns the rectangle for the main window.
    fn layout_grid(
        &self,
        layout: &Layout,
        client_layouts: &mut [ClientLayout],
        row_starts: &mut Vec<usize>,
        icons: Vec<bool>,
    ) -> Option<(Rectangle, bool)> {
        let mut previews = true;
        // TODO: could be better as this ignores the actual sizes of the
        // previews and just minimizes the empty cells on the last row.
        let mut options = Vec::with_capacity(client_layouts.len());
        for row_count in 1..=client_layouts.len() {
            let leftover = client_layouts.len() % row_count;
            options.push((leftover, row_count));
        }
        options.sort_unstable();
        let mut rows = Vec::new();
        let max_row_width = match try_distribute(client_layouts, layout, &mut rows, &options) {
            Some(width) => width,
            None => {
                previews = false;
                for client in client_layouts.iter_mut() {
                    client.disable_preview(layout.max_preview_width);
                }
                try_distribute(client_layouts, layout, &mut rows, &options)?
            }
        };
        let mut row_width;
        let mut y = layout.padding;
        for row in rows {
            row_width = row
                .iter()
                .map(|&index| client_layouts[index].width() + layout.spacing as u16)
                .sum::<u16>()
                - layout.spacing as u16;
            row_starts.push(row[0]);
            let mut x = layout.padding + (max_row_width - row_width) as i16 / 2;
            let mut height = 0;
            for index in row {
                let has_icon = icons[index];
                let client = &mut client_layouts[index];
                client.set_position(x, y, layout, has_icon);
                height = height.max(client.height());
                x += client.width() as i16 + layout.spacing;
            }
            y += height as i16 + layout.spacing;
        }
        let width = 2 * layout.padding as u16 + max_row_width;
        let height = (y - layout.spacing + layout.padding) as u16;
        let monitor = *monitors().primary().geometry();
        let x = monitor.x + (monitor.width - width) as i16 / 2;
        let y = monitor.y + (monitor.height - height) as i16 / 2;
        Some((Rectangle::new(x, y, width, height), previews))
    }

    /// Tries one layout, returns the geometry of the container window and
    /// whether this layout can have previews.  Returns `None` if this layout
    /// cannot be used at all even without preview and `Some((container_geometry,
    /// previews))` otherwise where `previews` is `true` if it can have previews.
    fn try_layout(
        &self,
        removed: XcbWindow,
        layout: &Layout,
        client_layouts: &mut Vec<ClientLayout>,
        row_starts: &mut Vec<usize>,
    ) -> Option<(Rectangle, bool)> {
        client_layouts.clear();
        let workspace = self.wm.active_workspace();
        for client in workspace.iter() {
            if client.handle() == removed {
                continue;
            }
            let (client_width, client_height) = client.client_geometry().size();
            let aspect_ratio = client_width as f64 / client_height as f64;
            client_layouts.push(layout.client(aspect_ratio, client.icon().is_some()));
        }
        let icons: Vec<_> = workspace
            .iter()
            .map(|client| client.icon().is_some())
            .collect();
        drop(workspace);
        let (geometry, previews) = self.layout_grid(layout, client_layouts, row_starts, icons)?;
        Some((geometry, previews))
    }

    /// Rebuilds the layout.  If `removed` is not `XcbWindow::none()` it will
    /// be ignored in the workspaces client list.
    fn layout(&mut self, removed: XcbWindow) -> bool {
        self.previews = true;
        self.clear_clients();
        // Determine layout
        let client_count = self.wm.active_workspace().clients().len();
        let mut client_layouts = Vec::with_capacity(client_count);
        let mut row_starts = Vec::with_capacity(client_count);
        let mut first_without_previews = None;
        let mut ok = false;
        for (index, layout) in self.layouts.iter().enumerate() {
            row_starts.clear();
            if let Some((container_geometry, previews)) =
                self.try_layout(removed, layout, &mut client_layouts, &mut row_starts)
            {
                self.geometry = container_geometry;
                if previews {
                    first_without_previews = None;
                    self.used_layout = index;
                    ok = true;
                    break;
                }
                if first_without_previews.is_none() {
                    first_without_previews = Some(index);
                }
            }
        }
        if let Some(index) = first_without_previews {
            self.previews = false;
            self.used_layout = index;
            row_starts.clear();
            (self.geometry, _) = self
                .try_layout(
                    removed,
                    &self.layouts[index],
                    &mut client_layouts,
                    &mut row_starts,
                )
                .unwrap();
            ok = true;
        }
        if !ok {
            log::error!("cannot fit the window switcher on the screen");
            self.cancel();
            return false;
        }
        // Build layout
        self.row_starts = row_starts;
        let make_input_window = |parent| {
            let win = InputOnlyWindow::builder()
                .with_parent(parent)
                .with_crossing()
                .with_mouse(true, true, false)
                .build(&self.wm.display);
            self.wm.set_window_kind(&win, WindowKind::WindowSwitcher);
            win
        };
        for (client, client_layout) in self
            .wm
            .active_workspace()
            .iter()
            .filter(|c| c.handle() != removed)
            .zip(client_layouts.into_iter())
        {
            let input_window = make_input_window(self.window.handle());
            let close_button = make_input_window(input_window.handle());
            let surface = create_xcb_surface(
                client.display(),
                client.window().resource_id(),
                &client.window().get_visual(),
                client.client_geometry().size(),
            );
            self.clients.push(WindowSwitcherClient {
                client: client.clone(),
                layout: client_layout,
                input_window,
                close_button,
                hovered: false,
                close_button_hovered: false,
                close_button_pressed: false,
                selected: false,
                surface,
            });
        }
        self.window.move_and_resize(self.geometry);
        // using `window.map_sub_windows()` causes the close_button to not
        // generate events, either because it's not being mapped for some reason
        // or the stacking order is wrong for some reason.
        for client in &self.clients {
            client.input_window.map(&self.wm.display);
            client.close_button.map(&self.wm.display);
            client.layout.layout_input_windows(&self.wm.display, client);
        }
        self.clients[self.switch_index].selected = true;
        true
    }

    /// Paints only the close button of a client.  If the client is not hovered
    /// it does nothing.
    fn paint_client_close_button(&self, dc: &DrawingContext, index: usize) {
        let c_bg = self.wm.config.colors.bar_background;
        let client = &self.clients[index];
        let c = if client.selected {
            self.wm.config.colors.selected_border()
        } else {
            self.wm.config.colors.normal_border()
        };
        if client.hovered {
            dc.rect(client.layout.background)
                .gradient(GradientSpec::new_vertical(c.top(), c.border()))
                .draw();
            dc.draw_colored_svg(
                self.wm.resources.close_button(),
                if client.close_button_hovered {
                    self.wm.config.colors.close_button_hovered
                } else {
                    self.wm.config.colors.close_button
                },
                client.layout.close_button,
            );
        } else {
            dc.fill_rect(client.layout.close_button, c_bg);
        }
    }

    /// Paints a single client, does not render and the font must already be set.
    fn paint_client(&self, dc: &DrawingContext, index: usize, repaint_background: bool) {
        let c_bg = self.wm.config.colors.bar_background;
        let c_norm_text = self.wm.config.colors.bar_text;
        let c_sel = self.wm.config.colors.selected_border();
        let c_sel_text = self.wm.config.colors.selected_text;
        let c_hov = self.wm.config.colors.normal_border();
        let c_hov_text = self.wm.config.colors.normal_text;
        #[allow(clippy::needless_late_init)]
        let c_text;
        let client = &self.clients[index];
        if repaint_background {
            dc.fill_rect(client.layout.background, c_bg);
        }
        if client.hovered || client.selected {
            let c = if client.selected { c_sel } else { c_hov };
            dc.rect(client.layout.background)
                .gradient(GradientSpec::new_vertical(c.top(), c.border()))
                .corner_radius(self.layouts[self.used_layout].client_padding as u16 * 3 / 2)
                .draw();
            c_text = if client.selected {
                c_sel_text
            } else {
                c_hov_text
            };
        } else {
            c_text = c_norm_text;
        }
        if let Some(icon) = client.client.icon() {
            dc.draw_svg(icon, client.layout.icon);
        }
        if let Some(title) = client.client.title() {
            dc.text(title, client.layout.title)
                .ellipsize(EllipsizeMode::End)
                .color(c_text)
                .vertical_alignment(Alignment::CENTER)
                .draw();
        }
        if client.hovered || client.selected {
            dc.draw_colored_svg(
                self.wm.resources.close_button(),
                if client.close_button_hovered {
                    self.wm.config.colors.close_button_hovered
                } else {
                    self.wm.config.colors.close_button
                },
                client.layout.close_button,
            );
        }
        if !self.previews {
            return;
        }
        if client.client.is_minimized() {
            if let Some(icon) = client.client.icon() {
                let rect = client.layout.preview;
                let smaller_side = u16::min(rect.width, rect.height);
                let icon_size = smaller_side * 90 / 100;
                let mut icon_rect = Rectangle::new(0, 0, icon_size, icon_size);
                icon_rect.center_inside(&rect);
                dc.draw_svg(icon, icon_rect);
            }
            return;
        }
        let (c_width, c_height) = client.client.client_geometry().size();
        let (p_width, p_height) = client.layout.preview.size();
        let x_scale = p_width as f64 / c_width as f64;
        let y_scale = p_height as f64 / c_height as f64;
        dc.draw_surface(
            &client.surface,
            x_scale,
            y_scale,
            client.layout.preview,
            self.wm.config.general.window_switcher_filter,
        );
    }

    /// Paints the background and all clients.
    fn paint(&self) {
        let c_bg = self.wm.config.colors.bar_background;
        let dc = self.wm.drawing_context.lock();
        dc.fill_rect(self.geometry.at(0, 0), c_bg);
        dc.set_font(&self.font);
        for i in 0..self.clients.len() {
            self.paint_client(&dc, i, false);
        }
        dc.render(&self.window, self.geometry.at(0, 0));
    }

    /// Resets the hovered index to `usize::MAX` and repaints the previously
    /// hovered client if there was one.
    fn clear_hovered(&mut self) {
        if self.hovered < self.clients.len() {
            self.clients[self.hovered].hovered = false;
            self.clients[self.hovered].close_button_hovered = false;
            let dc = self.wm.drawing_context.lock();
            dc.set_font(&self.font);
            self.paint_client(&dc, self.hovered, true);
            dc.render(&self.window, self.clients[self.hovered].layout.background);
        }
        self.hovered = usize::MAX;
    }

    /// Selects a client.
    fn select(&mut self, index: usize) {
        if index == self.switch_index {
            return;
        }
        self.clients[self.switch_index].selected = false;
        self.clients[index].selected = true;
        let dc = self.wm.drawing_context.lock();
        dc.set_font(&self.font);
        self.paint_client(&dc, self.switch_index, true);
        self.paint_client(&dc, index, true);
        dc.render(
            &self.window,
            self.clients[self.switch_index].layout.background,
        );
        dc.render(&self.window, self.clients[index].layout.background);
        self.switch_index = index;
    }

    /// Select a adjacent client depending on if delta is positive or not.
    fn select_next(&mut self, delta: i8) {
        let client_count = self.clients.len();
        let index;
        if delta < 0 {
            if self.switch_index == 0 {
                index = client_count - 1;
            } else {
                index = self.switch_index - 1;
            }
        } else if self.switch_index == client_count - 1 {
            index = 0;
        } else {
            index = self.switch_index + 1;
        }
        self.select(index);
    }

    /// Move the selection to an adjacent row.  If delta if negative the
    /// selection is moved up.
    fn select_next_row(&mut self, delta: i8) {
        let row_count = self.row_starts.len();
        if row_count == 0 {
            return;
        }
        let mut current_row = usize::MAX;
        for (row_index, &start_index) in self.row_starts.iter().enumerate() {
            if start_index > self.switch_index {
                current_row = row_index - 1;
                break;
            }
        }
        if current_row == usize::MAX {
            current_row = row_count - 1;
        }
        let position_in_row = self.switch_index - self.row_starts[current_row];
        let next_row = if delta < 0 {
            if current_row == 0 {
                row_count - 1
            } else {
                current_row - 1
            }
        } else {
            (current_row + 1) % row_count
        };
        let row_size = if next_row == row_count - 1 {
            self.clients.len() - self.row_starts[next_row]
        } else {
            self.row_starts[next_row + 1] - self.row_starts[next_row]
        };
        self.select(self.row_starts[next_row] + position_in_row.min(row_size - 1));
    }

    /// Focuses the selected client and destroys the window switcher.
    fn finish(&mut self) {
        let mut workspace = self.wm.active_workspace();
        workspace.no_focus = false;
        workspace.focus_at(self.switch_index);
        drop(workspace);
        self.destroy();
    }

    /// Destroys the window switcher without focusing a new client.
    fn cancel(&mut self) {
        let mut workspace = self.wm.active_workspace();
        workspace.no_focus = false;
        workspace.focus_at(0);
        drop(workspace);
        self.destroy();
    }

    /// Handles a `EnterNotifyEvent` or `LeaveNotifyEvent`.
    fn cross(&mut self, ev: &EnterNotifyEvent, is_enter: bool) {
        if !is_enter {
            if ev.event() == self.window.handle() {
                self.clear_hovered();
            } else {
                for client in self.clients.iter_mut() {
                    client.hovered = false;
                    client.close_button_hovered = false;
                }
            }
            return;
        }
        let mut hover_index = None;
        let mut close_button = false;
        for (index, client) in self.clients.iter_mut().enumerate() {
            if ev.event() == client.input_window.handle() {
                hover_index = Some(index);
                client.hovered = true;
                // `close_button` used to be set to the `client.close_button_hovered`
                // value but we can't do that anymore.
                close_button = true;
                client.close_button_hovered = false;
                break;
            } else if ev.event() == client.close_button.handle() {
                hover_index = Some(index);
                close_button = true;
                client.hovered = true;
                client.close_button_hovered = true;
                break;
            }
        }
        if let Some(hover_index) = hover_index {
            let dc = self.wm.drawing_context.lock();
            if hover_index != self.hovered {
                dc.set_font(&self.font);
                if self.hovered < self.clients.len() {
                    self.clients[self.hovered].hovered = false;
                    self.clients[self.hovered].close_button_hovered = false;
                    self.paint_client(&dc, self.hovered, true);
                    dc.render(&self.window, self.clients[self.hovered].layout.background);
                }
                self.paint_client(&dc, hover_index, false);
                dc.render(&self.window, self.clients[hover_index].layout.background);
            } else if close_button {
                self.paint_client_close_button(&dc, hover_index);
                dc.render(&self.window, self.clients[hover_index].layout.close_button);
            }
            self.hovered = hover_index;
        } else {
            self.clear_hovered();
        }
    }

    /// Handles a `ButtonPressEvent` or `ButtonReleaseEvent` event.
    fn click(&mut self, event: XcbWindow, down: bool) {
        let mut sel = None;
        for (index, client) in self.clients.iter_mut().enumerate() {
            if event == client.close_button.handle() {
                if !down && client.close_button_pressed && client.close_button_hovered {
                    close_client(&client.client);
                }
                client.close_button_pressed = down;
                return;
            } else if event == client.input_window.handle() && !down && client.hovered {
                sel = Some(index);
                break;
            }
        }
        if let Some(index) = sel {
            self.select(index);
            self.finish();
        }
    }

    /// Handles a `KeyPressEvent`.
    fn key_press(&mut self, event: &KeyPressEvent) {
        use x11::keysym::*;
        let sym = self.wm.display.keycode_to_keysym(event.detail());
        #[allow(non_upper_case_globals)]
        match sym as u32 {
            XK_Escape => self.cancel(),
            XK_Tab => self.select_next(if event.state().contains(self.shift) {
                -1
            } else {
                1
            }),
            XK_Right | XK_L | XK_l => self.select_next(1),
            XK_Left | XK_h | XK_H => self.select_next(-1),
            XK_Up | XK_K | XK_k => self.select_next_row(-1),
            XK_Down | XK_J | XK_j => self.select_next_row(1),
            XK_Return | XK_space => self.finish(),
            _ => {}
        }
    }

    /// Handles a `KeyReleaseEvent`.
    fn key_release(&mut self, event: &KeyReleaseEvent) {
        if event.detail() == self.wm.display.keysym_to_keycode(XK_Alt_L) {
            self.finish();
        }
    }

    /// Handles a change in client size.
    fn resize_client(&mut self, handle: XcbWindow) {
        for client in self.clients.iter() {
            if client.client.window().handle() == handle {
                let (width, height) = client.client.client_geometry().size();
                client
                    .surface
                    .set_size(width as i32, height as i32)
                    .unwrap();
            }
        }
    }
}

impl EventSink for WindowSwitcher {
    fn accept(&mut self, event: &xcb::Event) -> bool {
        self.in_signal_handler = false;
        use xcb::x::Event::*;
        let source = if let Some(source) = x_event_source(event) {
            source
        } else {
            return false;
        };
        if let WindowKind::WindowSwitcher = self.wm.get_window_kind(&source) {
            match event {
                Event::X(EnterNotify(ev)) => {
                    self.cross(ev, true);
                }
                Event::X(LeaveNotify(ev)) => {
                    self.cross(ev, false);
                }
                Event::X(KeyPress(ev)) => {
                    self.key_press(ev);
                }
                Event::X(KeyRelease(ev)) => {
                    self.key_release(ev);
                }
                Event::X(ButtonPress(ev)) => {
                    self.click(ev.event(), true);
                }
                Event::X(ButtonRelease(ev)) => {
                    self.click(ev.event(), false);
                }
                _ => {}
            }
            true
        } else {
            // We need to handle these here because alt+tab is grabbed by the
            // root window.  We could check for the root window but there is
            // not need to let any key presses past this.
            match event {
                Event::X(KeyPress(ev)) => {
                    self.key_press(ev);
                    return true;
                }
                Event::X(KeyRelease(ev)) => {
                    self.key_release(ev);
                    return true;
                }
                _ => {}
            }
            false
        }
    }

    fn signal(&mut self, signal: &Signal) {
        self.in_signal_handler = true;
        match signal {
            Signal::NewClient(_) => {
                self.layout(XcbWindow::none());
                self.window.raise();
                self.wm.display.set_input_focus(self.window.handle());
                self.paint();
            }
            Signal::ClientRemoved(handle) => {
                if self.clients.len() == 2 {
                    self.cancel();
                    return;
                }
                self.switch_index = self.switch_index.min(self.clients.len() - 1);
                self.layout(*handle);
                self.window.raise();
                self.wm.display.set_input_focus(self.window.handle());
                self.paint();
            }
            Signal::FocusClient(_) => {
                self.window.raise();
                self.wm.display.set_input_focus(self.window.handle());
            }
            Signal::ClientGeometry(handle, _) => {
                self.resize_client(*handle);
            }
            Signal::SnapStateChanged(handle, _, _) => {
                self.resize_client(*handle);
            }
            Signal::Quit => self.destroy(),
            _ => {}
        }
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        &[
            ButtonPressEvent::NUMBER,
            ButtonReleaseEvent::NUMBER,
            EnterNotifyEvent::NUMBER,
            KeyPressEvent::NUMBER,
            KeyReleaseEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
        ]
    }
}

/// Spawns a window switcher.  The active workspace must not be empty.
pub fn window_switcher(wm: Arc<WindowManager>) {
    let mut workspace = wm.active_workspace();
    workspace.no_focus = true;
    workspace.clients()[0].unfocus();
    workspace.clients()[0].draw_border();
    drop(workspace);
    let mut ws = Box::new(WindowSwitcher::new(wm.clone()));
    if ws.layout(XcbWindow::none()) {
        ws.window.map();
        ws.window.raise();
        ws.paint();
        wm.display.set_input_focus(ws.window.handle());
        wm.add_event_sink(SinkStorage::Unique(ws))
    }
}
