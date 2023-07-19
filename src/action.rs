use crate::{
    client::{Client, SetClientGeometry},
    error::OrFatal,
    event::Signal,
    ewmh::WindowState,
    log_error,
    monitors::{monitors, Monitor},
    process::{self, run_or_message_box},
    snap::SnapState,
    window_manager::WindowManager,
    window_switcher::window_switcher,
};
use xcb::Xid;

pub fn quit(wm: &WindowManager) {
    wm.quit(None);
}

pub fn quit_dialog() {
    run_or_message_box(&["window_manager_quit"]);
}

pub fn quit_dialog_action(_: &WindowManager) {
    quit_dialog();
}

pub fn close_client(client: &Client) {
    let display = client.display();
    if !client.send_message(display.atoms.wm_delete_window) {
        client.window().kill_client();
        display.flush();
    }
}

pub fn select_workspace(wm: &WindowManager, workspace_index: usize, _: Option<&Client>) {
    wm.set_workspace(workspace_index);
}

pub fn move_to_workspace(wm: &WindowManager, workspace_index: usize, client: Option<&Client>) {
    let client = client.unwrap();
    let arc = wm.active_workspace().remove(client);
    arc.unmap();
    wm.workspace(workspace_index).push(arc);
    let old = client.set_workspace(workspace_index);
    resnap(client);
    wm.signal_sender
        .send(Signal::ClientWorkspaceChanged(
            client.handle(),
            old,
            workspace_index,
        ))
        .or_fatal(&wm.display);
}

pub fn switch_window(wm: &WindowManager) {
    let ws = wm.active_workspace();
    if ws.clients().len() < 2 {
        return;
    }
    let wm = ws.clients()[0].get_window_manager();
    drop(ws);
    window_switcher(wm);
}

/// Snaps the client on the given monitor
pub fn snap_on_monitor(client: &Client, monitor: &Monitor, state: SnapState) {
    let splits = client.get_acting_splits();
    let geometry = state.get_geometry(splits, monitor, client.get_layout().gap());
    client.move_and_resize(SetClientGeometry::Snap(geometry));
    client.set_snap_state(state);
}

/// Snaps the client, using the given function to modify the snap state.
pub fn snap(client: &Client, f: impl Fn(&mut SnapState)) {
    let mon = client.get_monitor();
    let mut state = client.snap_state();
    f(&mut state);
    snap_on_monitor(client, &mon, state);
}

/// Snaps the client to its current snap state. Does nothing if the client
/// is not snapped.
/// Does not generate a `SnapStateChanged` signal.
pub fn resnap(client: &Client) {
    if !client.is_snapped() {
        return;
    }
    let mon = client.get_monitor();
    let splits = client.get_acting_splits();
    let geometry = client
        .snap_state()
        .get_geometry(splits, &mon, client.get_layout().gap());
    client.move_and_resize(SetClientGeometry::Snap(geometry));
}

pub fn snap_left(client: &Client) {
    snap(client, |state| state.snap_left());
}

pub fn snap_right(client: &Client) {
    snap(client, |state| state.snap_right());
}

pub fn snap_up(client: &Client) {
    snap(client, |state| state.snap_up());
}

pub fn snap_down(client: &Client) {
    snap(client, |state| state.snap_down());
}

pub fn maximize(client: &Client) {
    snap(client, |state| *state = SnapState::Maximized);
}

pub fn toggle_maximized(client: &Client) {
    if client.snap_state().is_maximized() {
        client.unsnap();
    } else {
        maximize(client);
    }
}

pub fn center(client: &Client) {
    let window_area = *monitors().containing(client).window_area();
    client.modify_saved_geometry(move |g| {
        g.center_inside(&window_area);
    });
    client.move_and_resize(SetClientGeometry::Frame(client.saved_geometry()));
}

pub fn unsnap_or_center(client: &Client) {
    if client.is_snapped() {
        client.unsnap();
    } else {
        center(client);
    }
}

pub fn minimize(client: &Client) {
    if client.state().is_fullscreen() {
        return;
    }
    client.unmap();
    client.set_state(WindowState::Minimized);
    client
        .get_window_manager()
        .signal_sender
        .send(Signal::ClientMinimized(client.handle(), true))
        .or_fatal(client.display());
    let wm = client.get_window_manager();
    let mut active_workspace = wm.active_workspace();
    if active_workspace.index() == client.workspace() {
        if let Some(focused) = active_workspace.focused().cloned() {
            active_workspace.focus(focused.handle());
        } else {
            wm.root.set_focused_client(None);
        }
    }
}

pub fn raise_all(wm: &WindowManager) {
    for client in wm.active_workspace().iter() {
        if client.state().is_minimized() {
            client.unminimize();
        }
    }
}

pub fn move_to_monitor(client: &Client, cur: &Monitor, mon: &Monitor) {
    if cur.index() == mon.index() {
        return;
    }
    let cg = cur.geometry();
    let ng = mon.geometry();
    let mut g = client.saved_geometry();
    g.x = g.x - cg.x + ng.x;
    g.y = g.y - cg.y + ng.y;
    g.clamp_inside(mon.window_area());
    if client.is_snapped() {
        client.modify_saved_geometry(|sg| {
            *sg = g;
        });
        client.set_monitor(mon.index() as isize, client.frame_geometry());
        resnap(client);
    } else {
        client.move_and_resize(SetClientGeometry::Frame(g));
        client.save_geometry();
    }
}

pub fn move_to_next_monitor(client: &Client) {
    if !client.may_move() {
        return;
    }
    let monitors = monitors();
    let current = monitors.containing(client);
    move_to_monitor(client, current, monitors.get(current.index() as isize + 1));
}

pub fn move_to_prev_monitor(client: &Client) {
    if !client.may_move() {
        return;
    }
    let monitors = monitors();
    let current = monitors.containing(client);
    move_to_monitor(client, current, monitors.get(current.index() as isize - 1));
}

pub fn grid_resize(client: &Client) {
    let config = &client.get_window_manager().config;
    let area = *client.get_monitor().window_area();
    let dimensions = format!("{},{},{},{}", area.x, area.y, area.width, area.height);
    let (vertical_cells, horizontal_cells) = config.general.grid_resize_grid_size;
    let cells = format!("{},{}", vertical_cells, horizontal_cells);
    let color = config.colors.selected;
    let color = format!("--color={},{},{}", color.red, color.green, color.blue);
    let window = format!("{}", client.window().resource_id());
    let mut cmd = vec![
        "grid-resize",
        &window,
        &dimensions,
        &cells,
        &color,
        "--method=message",
        "--right-button-pressed",
    ];
    if config.general.grid_resize_live {
        cmd.push("--live");
    }
    log_error!(process::run(cmd.as_slice()));
}
