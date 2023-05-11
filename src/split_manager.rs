use crate::{
    client::SetClientGeometry,
    draw::DrawingContext,
    event::{EventSink, Signal},
    monitors::monitors,
    snap::SnapState,
    split_handles::{Role, SplitHandle, SplitHandles},
    window_manager::{WindowKind, WindowManager},
    x::XcbWindow,
};
use parking_lot::Mutex;
use std::sync::{Arc, Weak};
use std::{
    collections::BTreeMap,
    ops::{Add, Sub},
};
use xcb::{
    x::{ButtonPressEvent, EnterNotifyEvent, LeaveNotifyEvent},
    Event,
};

#[derive(Default)]
pub struct SplitManager {
    /// Outer vector is per-workspace, inner vectors are per-monitor.
    handles: Vec<Vec<SplitHandles>>,
    wm: Weak<WindowManager>,
    dc: Weak<Mutex<DrawingContext>>,
    current_workspace: usize,
    /// For the active workspaces, maps window handles of handles to their
    /// monitor index and their role on that monitor.
    active_window_handles: Vec<(XcbWindow, (isize, Role))>,
}

impl SplitManager {
    /// Constructs the object after being created using the `default` function.
    pub fn construct(&mut self, wm: &Arc<WindowManager>) {
        self.wm = Arc::downgrade(wm);
        self.dc = Arc::downgrade(&wm.drawing_context);
        self.current_workspace = wm.active_workspace_index();
        let monitors = monitors();
        for workspace in 0..wm.config.layout.workspaces {
            self.handles.push(Vec::with_capacity(monitors.len()));
            for monitor in monitors.iter() {
                self.handles[workspace].push(SplitHandles::new(wm, monitor));
            }
        }
        self.set_active_windows();
        for monitor in 0..monitors.len() {
            self.active(monitor as isize).visible(true);
        }
    }

    fn destroy(&self) {
        let wm = self.wm.upgrade().unwrap();
        for workspace in self.handles.iter() {
            for handles in workspace.iter() {
                handles.destroy(&wm);
            }
        }
    }

    /// Populates the `active_window_handles` vector.
    fn set_active_windows(&mut self) {
        self.active_window_handles.clear();
        let ws = &self.handles[self.current_workspace];
        for (monitor, split_handles) in ws.iter().enumerate() {
            for role in [Role::Vertical, Role::Left, Role::Right] {
                let handle = split_handles.handle(role);
                self.active_window_handles
                    .push((handle.window().handle(), (monitor as isize, role)));
            }
        }
    }

    pub fn get_workspace(&self, workspace: usize) -> &Vec<SplitHandles> {
        &self.handles[workspace]
    }

    pub fn get_handles(&self, workspace: usize, monitor: isize) -> &SplitHandles {
        &self.get_workspace(workspace)[monitor as usize]
    }

    pub fn active(&self, monitor: isize) -> &SplitHandles {
        self.get_handles(self.current_workspace, monitor)
    }

    fn find_active_handle_index(&self, window: XcbWindow) -> Option<usize> {
        self.active_window_handles
            .iter()
            .position(|(h, _)| *h == window)
    }

    fn find_handle(&self, window: XcbWindow) -> Option<&SplitHandle> {
        let idx = self.find_active_handle_index(window)?;
        let (monitor, role) = self.active_window_handles[idx].1;
        Some(self.active(monitor).handle(role))
    }

    fn enter(&self, e: &EnterNotifyEvent) {
        let handle = self.find_handle(e.event()).unwrap();
        let dc = self.dc.upgrade().unwrap();
        let dc = dc.lock();
        handle.raise_and_draw_hovered(&dc);
    }

    fn leave(&self, e: &LeaveNotifyEvent) {
        // When changing the workspace while hovering a split handle we will
        // process the workspace change first and then get the `LeaveNotify`
        // event which we can now no longer find the handle for.
        if let Some(handle) = self.find_handle(e.event()) {
            handle.lower_and_clear();
        }
    }

    fn click(&mut self, e: &ButtonPressEvent) {
        let wm = self.wm.upgrade().unwrap();
        let idx = self.find_active_handle_index(e.event()).unwrap();
        let (monitor, role) = self.active_window_handles[idx].1;
        let handle = self.handles[self.current_workspace][monitor as usize].handle_mut(role);
        if let Some(position) = handle.mouse_move(e.event_x(), e.event_y(), &wm) {
            self.handles[self.current_workspace][monitor as usize].update(role, position);
        }
        let splits = self.handles[self.current_workspace][monitor as usize].as_splits();
        for client in wm.workspace(self.current_workspace).iter() {
            if client.monitor() == monitor {
                // Copy of `action::resnap` but we cannot used that as it would
                // require borrowing the split manager to get the splits.
                let mon = client.get_monitor();
                let geometry =
                    client
                        .snap_state()
                        .get_geometry(splits, &mon, client.get_layout().gap());
                client.move_and_resize(SetClientGeometry::Snap(geometry));
            }
        }
    }

    fn change_count(
        &mut self,
        workspace: usize,
        monitor: isize,
        state: SnapState,
        op: fn(u16, u16) -> u16,
    ) {
        let handles = &mut self.handles[workspace][monitor as usize];
        if state.is_snapped() && !state.is_maximized() {
            handles.vertical_clients = op(handles.vertical_clients, 1);
        }
        match state {
            SnapState::TopLeft | SnapState::BottomLeft => {
                handles.left_clients = op(handles.left_clients, 1);
            }
            SnapState::TopRight | SnapState::BottomRight => {
                handles.right_clients = op(handles.right_clients, 1);
            }
            _ => {}
        }
    }

    fn resize(&mut self) {
        self.destroy();
        let monitors = monitors();
        let default_splits = (0.5, 0.5, 0.5);
        let wm = self.wm.upgrade().unwrap();
        for workspace in self.handles.iter_mut() {
            let old: BTreeMap<String, (f64, f64, f64)> = workspace
                .iter()
                .map(|handles| {
                    let monitor = handles.monitor_name().to_owned();
                    let splits = handles.split_percentages();
                    (monitor, splits)
                })
                .collect();
            workspace.clear();
            for monitor in monitors.iter() {
                workspace.push(SplitHandles::with_percentages(
                    &wm,
                    monitor,
                    old.get(monitor.name()).unwrap_or(&default_splits),
                ));
            }
        }
        self.set_active_windows();
        for workspace_index in 0..self.handles.len() {
            let workspace = wm.workspace(workspace_index);
            for client in workspace.iter() {
                self.change_count(
                    workspace_index,
                    client.monitor(),
                    client.snap_state(),
                    u16::add,
                );
            }
            if workspace_index == self.current_workspace {
                for handles in self.handles[workspace_index].iter() {
                    handles.visible(true);
                }
            }
        }
    }
}

impl EventSink for SplitManager {
    fn accept(&mut self, event: &Event) -> bool {
        use xcb::x::Event::*;
        let wm = self.wm.upgrade().unwrap();
        if !wm.source_kind_matches(event, WindowKind::SplitHandle) {
            return false;
        }
        match event {
            Event::X(EnterNotify(e)) => self.enter(e),
            Event::X(LeaveNotify(e)) => self.leave(e),
            Event::X(ButtonPress(e)) => self.click(e),
            _ => {}
        }
        true
    }

    fn signal(&mut self, signal: &Signal) {
        let wm = self.wm.upgrade().unwrap();
        match signal {
            Signal::SnapStateChanged(handle, old, new) => {
                let client = wm.win2client(handle).unwrap();
                let workspace = client.workspace();
                let monitor = client.monitor();
                self.change_count(workspace, monitor, *new, u16::add);
                self.change_count(workspace, monitor, *old, u16::sub);
                if workspace == self.current_workspace {
                    self.active(monitor).visible(true);
                }
            }
            Signal::ClientMonitorChanged(handle, old, new) => {
                let client = wm.win2client(handle).unwrap();
                let workspace = client.workspace();
                let state = client.snap_state();
                self.change_count(workspace, *old, state, u16::sub);
                self.change_count(workspace, *new, state, u16::add);
                if workspace == self.current_workspace {
                    self.active(*old).visible(true);
                    self.active(*new).visible(true);
                }
            }
            Signal::ClientWorkspaceChanged(handle, old, new) => {
                let client = wm.win2client(handle).unwrap();
                let monitor = client.monitor();
                let state = client.snap_state();
                self.change_count(*new, monitor, state, u16::add);
                self.change_count(*old, monitor, state, u16::sub);
                if *new == self.current_workspace || *old == self.current_workspace {
                    self.active(monitor).visible(true);
                }
            }
            Signal::ClientRemoved(handle) => {
                let client = wm.win2client(handle).unwrap();
                let workspace = client.workspace();
                let monitor = client.monitor();
                self.change_count(workspace, monitor, client.snap_state(), u16::sub);
                if workspace == self.current_workspace {
                    self.active(monitor).visible(true);
                }
            }
            Signal::WorkspaceChanged(old, new) => {
                for i in self.get_workspace(*old).iter() {
                    i.visible(false);
                }
                for i in self.get_workspace(*new).iter() {
                    i.visible(true);
                }
                self.current_workspace = *new;
                self.set_active_windows();
            }
            Signal::Resize => self.resize(),
            Signal::Quit => self.destroy(),
            _ => {}
        }
    }

    fn filter(&self) -> &'static [u32] {
        use xcb::{x::*, BaseEvent};
        const FILTER: [u32; 3] = [
            EnterNotifyEvent::NUMBER,
            LeaveNotifyEvent::NUMBER,
            ButtonPressEvent::NUMBER,
        ];
        return &FILTER;
    }
}
