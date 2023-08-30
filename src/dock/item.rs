use super::dock::ItemRef;
use crate::{
    action,
    client::Client,
    color::Color,
    context_menu::Indicator,
    desktop_entry::DesktopEntry,
    draw::{self, ColorKind, DrawingContext, Svg},
    error::message_box,
    event::SinkStorage,
    process::{run_or_message_box, split_commandline},
    rectangle::{Rectangle, ShowAt},
    tooltip::Tooltip,
    window_manager::{WindowKind, WindowManager},
    x::{InputOnlyWindow, Window, XcbWindow},
};
use std::{rc::Rc, sync::Arc};

type ContextMenu = crate::context_menu::ContextMenu<ItemRef>;

fn get_icon(entry: Option<&DesktopEntry>, icon_theme: &str) -> Option<(Svg, bool)> {
    let maybe_name_or_path = entry.and_then(|e| e.icon.clone());
    if let Some(app_icon) = maybe_name_or_path.and_then(|name| {
        let icon_path = if name.starts_with('/') {
            name
        } else {
            format!("{}/48x48/apps/{}.svg", icon_theme, name)
        };
        Svg::try_load(&icon_path).ok()
    }) {
        Some((app_icon, true))
    } else if let Some(Ok(icon)) = draw::load_icon("applications-system", icon_theme) {
        Some((icon, false))
    } else {
        None
    }
}

fn get_title_and_unsaved_changes(mut title: String) -> (String, bool) {
    // As far as I can tell there is no property or other way for windows to
    // signal that they have unsaved changes so we look for common indicators
    // in the window title.
    // If we find such an indicator it is removed from the returned title.
    let unsaved_indicators = &["*", "●", "+"];
    let mut has_unsaved = false;
    for indicator in unsaved_indicators {
        if title.starts_with(indicator) {
            title.remove(0);
            title = title.trim_start().to_string();
            has_unsaved = true;
            break;
        } else if title.ends_with(indicator) {
            title.pop();
            title = title.trim_end().to_string();
            has_unsaved = true;
            break;
        }
    }
    (title, has_unsaved)
}

pub struct Item {
    id: String,
    desktop_entry: Option<DesktopEntry>,
    is_pinned: bool,
    action_icons: Vec<Option<Rc<Svg>>>,
    instances: Vec<Arc<Client>>,
    window: InputOnlyWindow,
    icon: Svg,
    command: Vec<String>,
    focused_instance: usize,
    has_urgent: bool,
    is_hovered: bool,
    tooltip: Tooltip,
    geometry: Rectangle,
    icon_rect: Rectangle,
    wm: Arc<WindowManager>,
    context_menu_open: bool,
    indicator_color: Color,
}

impl Item {
    pub fn new(
        dock_window: &Window,
        name: &str,
        is_pinned: bool,
        geometry: Rectangle,
        icon_rect: Rectangle,
        wm: &Arc<WindowManager>,
    ) -> Option<Self> {
        let id = DesktopEntry::entry_name(name).unwrap_or_else(|| name.to_string());
        let de = DesktopEntry::new(&id);
        if de.is_none() && is_pinned {
            message_box(
                "Application not found",
                &format!("'{}' was not found and got removed from the dock", name),
            );
            return None;
        }
        let window = InputOnlyWindow::builder()
            .with_parent(dock_window.handle())
            .with_geometry(geometry)
            .with_mouse(true, false, false)
            .with_crossing()
            .build(&wm.display);
        wm.set_window_kind(&window, WindowKind::DockItem);
        window.map(&wm.display);
        let (icon, is_app_icon) = if let Some(icon) = get_icon(de.as_ref(), &wm.config.icon_theme) {
            icon
        } else {
            message_box(
                &format!("No suitable icon found for '{}'", name),
                "It got removed from the dock",
            );
            return None;
        };
        let mut action_icons = Vec::new();
        if let Some(de) = &de {
            for action in de.actions.iter() {
                action_icons.push(action.icon.as_ref().and_then(|name| {
                    Some(Rc::new(draw::load_icon(name, &wm.config.icon_theme)?.ok()?))
                }));
            }
        }
        let command = if let Some(de) = &de {
            // TODO: why is this unwrapped?
            split_commandline(de.exec.as_ref().unwrap())
        } else {
            Vec::new()
        };
        let indicator_color = if is_app_icon && wm.config.dock.auto_indicator_colors {
            wm.drawing_context
                .lock()
                .get_average_svg_color(&icon, (0, 0, 100, 100))
        } else {
            wm.config.colors.dock_indicator
        };
        Some(Self {
            id,
            desktop_entry: de,
            is_pinned,
            action_icons,
            instances: Vec::new(),
            window,
            icon,
            command,
            focused_instance: 0,
            has_urgent: false,
            is_hovered: false,
            tooltip: Tooltip::new(wm),
            geometry,
            icon_rect,
            wm: wm.clone(),
            context_menu_open: false,
            indicator_color,
        })
    }

    pub fn set_geometry(&mut self, geometry: Rectangle) {
        self.window.move_and_resize(&self.wm.display, geometry);
        self.geometry = geometry;
    }

    pub fn set_icon_rect(&mut self, rect: Rectangle) {
        self.icon_rect = rect;
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn window(&self) -> &InputOnlyWindow {
        &self.window
    }

    pub fn name(&self) -> &str {
        self.desktop_entry
            .as_ref()
            .map(|de| de.name.as_str())
            .unwrap_or(self.id.as_str())
    }

    pub fn is_hovered(&mut self, is_hovered: bool) {
        self.is_hovered = is_hovered;
    }

    pub fn update_urgency(&mut self) -> bool {
        let before = self.has_urgent;
        self.has_urgent = false;
        for client in self.instances.iter() {
            if client.is_urgent() {
                self.has_urgent = true;
                break;
            }
        }
        before != self.has_urgent
    }

    pub fn update(&self, window: &Window, dc: &DrawingContext, from_dock_update: bool) {
        if !from_dock_update {
            // Clear background if not coming from a full dock update
            dc.fill_rect(self.geometry, self.wm.config.colors.dock_background);
        }
        if self.is_hovered || self.has_urgent {
            // Hover/urgent background
            let color = if self.is_hovered {
                self.wm.config.colors.dock_hovered
            } else {
                self.wm.config.colors.dock_urgent
            };
            dc.rect(self.geometry)
                .corner_percent(0.1)
                .color(color)
                .stroke(1, ColorKind::Solid(color.scale(1.333)))
                .draw();
        }
        // Icon
        let mut icon_rect = self.icon_rect;
        icon_rect.x += self.geometry.x;
        icon_rect.y += self.geometry.y;
        dc.draw_svg(&self.icon, icon_rect);
        if !self.instances.is_empty() {
            // Indicator for active instances
            let height = self.geometry.height / 16;
            let width = self.geometry.width / 4;
            let x = self.geometry.x + (self.geometry.width - width) as i16 / 2;
            let y = self.geometry.y + (self.geometry.height - height) as i16;
            dc.rect((x, y, width, height))
                .color(self.indicator_color)
                .corner_percent(0.49)
                .draw();
        }
        if !from_dock_update {
            // Render if not coming from a dock update
            dc.render(window, self.geometry);
        }
    }

    pub fn contains(&self, handle: XcbWindow) -> bool {
        self.instances.iter().any(|c| c.handle() == handle)
    }

    pub fn show_tooltip(&self, dock_geometry: Rectangle) {
        if self.context_menu_open {
            return;
        }
        let x_in_dock = self.geometry.x + self.geometry.width as i16 / 2;
        let y_in_dock = self.geometry.y;
        let x = dock_geometry.x + x_in_dock;
        let y = dock_geometry.y + y_in_dock;
        self.tooltip.show(self.name(), ShowAt::BottomCenter((x, y)));
    }

    pub fn hide_tooltip(&self) {
        self.tooltip.close();
    }

    pub fn add_instance(&mut self, client: Arc<Client>) {
        let handle = client.handle();
        self.instances.push(client);
        self.update_focus(handle);
    }

    fn find_instance(&self, handle: XcbWindow) -> Option<usize> {
        self.instances.iter().position(|c| c.handle() == handle)
    }

    /// Returns whether the item should be removed.
    pub fn remove_instance(&mut self, handle: XcbWindow) -> bool {
        if let Some(idx) = self.find_instance(handle) {
            let c = self.instances.remove(idx);
            if c.is_urgent() {
                c.set_urgency(false);
                // We don't update self.has_urgent here as the `set_urgency`
                // function of the client generates a `UrgencyChanged` signal.
            }
        }
        self.instances.is_empty() && !self.is_pinned
    }

    pub fn launch_new_instance(&self) {
        if !self.command.is_empty() {
            run_or_message_box(&self.command);
        }
    }

    fn focus_instance(&mut self, idx: usize) {
        if idx >= self.instances.len() {
            log::error!(
                "dock::Item::focus_instance: idx is out of bounds: idx={} instances={}",
                idx,
                self.instances.len()
            );
            let backtrace = std::backtrace::Backtrace::force_capture();
            log::error!("Backtrace:\n{backtrace}");
            if cfg!(debug_assertions) {
                self.wm.notify(
                    "Internal error",
                    "dock::Item::focus_instance index is out of bounds.
                    See log for more information.",
                    "dialog-error",
                    0,
                );
            }
            return;
        }
        self.focused_instance = idx;
        let client = &self.instances[idx];
        if client.workspace() != self.wm.active_workspace_index() {
            action::select_workspace(&self.wm, client.workspace(), None);
        }
        self.wm.active_workspace().focus(client.handle());
    }

    pub fn click(&mut self) {
        if self.instances.is_empty() {
            self.launch_new_instance();
            return;
        }
        if self.has_urgent && self.wm.config.dock.focus_urgent {
            for (idx, instance) in self.instances.iter().enumerate() {
                if instance.is_urgent() {
                    self.focus_instance(idx);
                    return;
                }
            }
        }
        self.focus_instance(self.focused_instance);
    }

    pub fn update_focus(&mut self, handle: XcbWindow) {
        if let Some(idx) = self.find_instance(handle) {
            if self.wm.config.dock.focused_client_on_top {
                let c = self.instances.remove(idx);
                self.instances.insert(0, c);
                self.focused_instance = 0;
            } else {
                self.focused_instance = idx;
            }
        }
    }

    fn add_instances_to_menu(&self, menu: &mut ContextMenu) {
        if self.instances.is_empty() {
            return;
        }
        let mut all_on_current_workspace = false;
        let active_workspace = self.wm.active_workspace_index();
        for c in self.instances.iter() {
            if c.workspace() != active_workspace {
                all_on_current_workspace = false;
                break;
            }
        }
        for (index, client) in self.instances.iter().cloned().enumerate() {
            let (title, unsaved_changes) = if let Some(title) = client.title() {
                get_title_and_unsaved_changes(title.to_string())
            } else {
                ("?".to_string(), false)
            };
            let indicator = if index == self.focused_instance {
                Some(Indicator::Check)
            } else if client.is_urgent() {
                Some(Indicator::Exclamation)
            } else if unsaved_changes {
                Some(Indicator::Circle)
            } else if client.is_minimized() {
                Some(Indicator::Diamond)
            } else {
                None
            };
            let info = if self.wm.config.dock.context_show_workspaces && !all_on_current_workspace {
                format!(" ({})", client.workspace() + 1)
            } else {
                String::new()
            };
            menu.add_action(
                title,
                Box::new(move |mut self_ref| {
                    self_ref.get().focus_instance(index);
                }),
            )
            .indicator(indicator)
            .icon(client.icon().cloned())
            .info(info);
        }
        menu.add_divider();
    }

    fn add_actions_to_menu(&self, menu: &mut ContextMenu) {
        if self.action_icons.is_empty() {
            return;
        }
        if let Some(de) = &self.desktop_entry {
            for (idx, action) in de.actions.iter().enumerate() {
                menu.add_action(
                    action.name.to_string(),
                    Box::new(move |mut self_ref| {
                        let action = &self_ref.get().desktop_entry.as_ref().unwrap().actions[idx];
                        let command: Vec<String> =
                            split_commandline(action.exec.as_deref().unwrap());
                        run_or_message_box(&command);
                    }),
                )
                .icon(self.action_icons[idx].clone());
            }
            menu.add_divider();
        }
    }

    fn add_default_actions(&self, menu: &mut ContextMenu) {
        if !self.command.is_empty() {
            menu.add_action(
                "Launch",
                Box::new(|mut self_ref| {
                    self_ref.get().launch_new_instance();
                }),
            );
        }
        if let Some(focused_) = self.instances.get(self.focused_instance) {
            let focused = focused_.clone();
            if focused.is_minimized() {
                menu.add_action(
                    "Show",
                    Box::new(move |_| {
                        focused.unminimize();
                    }),
                );
            } else {
                menu.add_action(
                    "Hide",
                    Box::new(move |_| {
                        action::minimize(&focused);
                    }),
                );
            }
            let focused = focused_.clone();
            menu.add_action(
                "Close",
                Box::new(move |_| {
                    action::close_client(&focused);
                }),
            );
        }
    }

    pub fn context_menu(&self, dock_geometry: Rectangle, mut self_ref: ItemRef) -> XcbWindow {
        let wm = self.wm.clone();
        let mut menu = ContextMenu::new(self.wm.clone(), self_ref);
        menu.after(Box::new(move |mut self_ref| {
            self_ref.get().context_menu_open = false;
            if !wm.active_workspace().is_empty() {
                let dock = self_ref.get_dock();
                dock.keep_open(false);
            }
        }));
        self.add_instances_to_menu(&mut menu);
        self.add_actions_to_menu(&mut menu);
        self.add_default_actions(&mut menu);
        menu.show_at(ShowAt::BottomCenter((
            dock_geometry.x + self.geometry.x + self.geometry.width as i16 / 2,
            dock_geometry.y + self.geometry.y - 5,
        )));
        self_ref.get().context_menu_open = true;
        self.hide_tooltip();
        let handle = menu.window().handle();
        self.wm.add_event_sink(SinkStorage::Unique(Box::new(menu)));
        handle
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        self.wm.remove_all_contexts(&self.window);
        self.window.destroy(&self.wm.display);
    }
}
