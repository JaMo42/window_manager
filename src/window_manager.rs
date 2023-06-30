use crate::{
    action::close_client,
    appinfo::set_startup_wm_classes,
    bar,
    client::Client,
    config::{Action, Config},
    context::{Context, ContextMap},
    cursor::Cursors,
    dbus::DBusConnection,
    dock::Dock,
    draw::{BuiltinResources, DrawingContext},
    error::OrFatal,
    event::{x_event_source, Signal, SinkId, SinkStorage},
    event_router::EventRouter,
    ewmh::Root,
    log_error,
    main_event_sink::MainEventSink,
    monitors::monitors,
    notifications::{NotificationManager, NotificationProps},
    paths, platform,
    process::run_and_await,
    session_manager::SessionManager,
    split_manager::SplitManager,
    volume::{get_audio_api, AudioAPI},
    workspace::Workspace,
    x::{close_window, Display, ModifierMapping, PropertyValue, SetProperty, Window, XcbWindow},
    AnyResult,
};
use glib::subclass::shared::RefCounted;
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use std::{
    cell::{Cell, Ref, RefCell},
    cmp::Ordering,
    collections::HashMap,
    mem::discriminant,
    rc::Rc,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};
use xcb::{
    x::{ButtonIndex, EventMask, KeyPressEvent, Mapping, ModMask, GRAB_ANY},
    Event, Xid,
};

pub const NAME: &str = "window_manger";

#[derive(Copy, Clone, Debug)]
#[repr(usize)]
pub enum WindowKind {
    Client,
    ContextMenu,
    ContextMenuItem,
    Dock,
    DockItem,
    DockShow,
    ExtendedFrame,
    Frame,
    FrameButton,
    MetaOrUnmanaged,
    MouseBlock,
    Notification,
    Root,
    SplitHandle,
    StatusBar,
    StatusBarWidget,
    TrayClient,
}

pub struct WindowManager {
    pub display: Arc<Display>,
    pub config: Arc<Config>,
    pub drawing_context: Arc<Mutex<DrawingContext>>,
    pub modmap: RefCell<ModifierMapping>,
    pub root: Root,
    pub signal_sender: Sender<Signal>,
    pub resources: Arc<BuiltinResources>,
    pub cursors: Arc<Cursors>,
    pub dbus: DBusConnection,
    pub session_manager: Arc<Mutex<SessionManager>>,
    signal_receiver: Receiver<Signal>,
    context_map: Mutex<ContextMap>,
    event_sinks: RefCell<EventRouter>,
    is_running: Cell<bool>,
    unmanaged: RefCell<Vec<Window>>,
    workspaces: Arc<Mutex<Vec<Workspace>>>,
    active_workspace: Arc<Mutex<usize>>,
    quit_reason: RefCell<Option<String>>,
    notification_manager: Arc<Mutex<NotificationManager>>,
    split_manager: Rc<RefCell<SplitManager>>,
    remove_sinks: RefCell<Vec<usize>>,
    // need this on the window manager so it can be accessed for the audio
    // control key bindings.
    audio_api: Option<Mutex<Box<dyn AudioAPI>>>,
}

unsafe impl Send for WindowManager {}
unsafe impl Sync for WindowManager {}

impl WindowManager {
    pub fn new(display: Arc<Display>) -> Arc<Self> {
        let root = Root(Window::from_handle(display.clone(), display.root()));
        let modmap = RefCell::new(ModifierMapping::new().refresh(&display));
        let drawing_context = Arc::new(Mutex::new(DrawingContext::create(
            display.clone(),
            display.get_total_size(),
        )));
        let config = Arc::new(Config::load(&display, &drawing_context.lock()));
        let cursors = Arc::new(Cursors::create(display.clone()));
        let resources = Arc::new(BuiltinResources::load_all());
        let (signal_sender, signal_receiver) = channel();
        let dbus = DBusConnection::new().unwrap_or_fatal(&display);
        let this = Arc::new(Self {
            display: display.clone(),
            config,
            drawing_context,
            modmap,
            root,
            signal_sender,
            resources,
            cursors,
            signal_receiver,
            context_map: Mutex::new(ContextMap::new()),
            event_sinks: RefCell::new(EventRouter::new()),
            is_running: Cell::new(true),
            unmanaged: RefCell::new(Vec::new()),
            workspaces: Arc::new(Mutex::new(Vec::new())),
            active_workspace: Arc::new(Mutex::new(0)),
            dbus,
            quit_reason: RefCell::new(None),
            session_manager: Arc::new(Mutex::new(SessionManager::new())),
            notification_manager: Arc::new(Mutex::new(NotificationManager::new())),
            split_manager: Rc::new(RefCell::new(SplitManager::default())),
            remove_sinks: RefCell::new(Vec::with_capacity(4)),
            audio_api: get_audio_api().map(Mutex::new),
        });
        this.session_manager.lock().set_window_manager(&this);
        this.notification_manager.lock().set_window_manager(&this);
        this.split_manager.borrow_mut().construct(&this);
        SessionManager::register(this.session_manager.clone()).unwrap_or_fatal(&display);
        NotificationManager::register(this.notification_manager.clone()).unwrap_or_fatal(&display);
        let main_event_sink = Box::new(MainEventSink::new(this.clone()));
        {
            let mut e = this.event_sinks.borrow_mut();
            e.add(SinkStorage::Unique(main_event_sink));
            e.add(SinkStorage::Shared(this.split_manager.clone()));
            e.add(SinkStorage::Mutex(this.notification_manager.clone()));
        }
        this
    }

    /// Returns the focused client on the current workspace, if there is one.
    pub fn focused_client(&self) -> Option<Arc<Client>> {
        self.active_workspace().focused().cloned()
    }

    pub fn split_manager(&self) -> Ref<SplitManager> {
        self.split_manager.borrow()
    }

    /// Get the shared active workspace index instance.
    pub fn get_active_workspace(&self) -> Arc<Mutex<usize>> {
        self.active_workspace.clone()
    }

    /// Get the index of the active workspace
    pub fn active_workspace_index(&self) -> usize {
        *self.active_workspace.lock()
    }

    /// Returns a reference to the workspace with the given index.
    pub fn workspace(&self, index: usize) -> MappedMutexGuard<Workspace> {
        let lock = self.workspaces.lock();
        MutexGuard::map(lock, |workspaces| &mut workspaces[index])
    }

    /// Returns a reference to the active workspace.
    pub fn active_workspace(&self) -> MappedMutexGuard<Workspace> {
        self.workspace(self.active_workspace_index())
    }

    /// Changes the active workspace.
    /// Emits a `WorkspaceChanged` signal.
    pub fn set_workspace(&self, idx: usize) {
        let mut active_workspace = self.active_workspace.lock();
        let from = *active_workspace;
        *active_workspace = idx;
        drop(active_workspace);
        if from == idx {
            return;
        }
        let mut workspaces = self.workspaces.lock();
        let empty_before = workspaces[from].is_empty();
        for old in workspaces[from].iter() {
            old.unmap();
        }
        for current in workspaces[idx].iter() {
            if !current.real_state().is_minimized() {
                current.map();
                current.draw_border();
            }
        }
        workspaces[from].set_active(false);
        workspaces[idx].set_active(true);
        drop(workspaces);
        if let Some(focused) = self.focused_client() {
            focused.focus();
            if empty_before {
                self.signal_sender
                    .send(Signal::ActiveWorkspaceEmpty(false))
                    .or_fatal(&self.display);
            }
        } else {
            self.root
                .delete_property(&self.display, self.display.atoms.net_active_window);
            if !empty_before {
                self.signal_sender
                    .send(Signal::ActiveWorkspaceEmpty(true))
                    .or_fatal(&self.display);
            }
        }
        self.root.set_property(
            &self.display,
            self.display.atoms.net_active_window,
            PropertyValue::Cardinal(idx as u32),
        );
        self.signal_sender
            .send(Signal::WorkspaceChanged(from, idx))
            .or_fatal(&self.display);
    }

    /// Logs workspace contents if debug assertions are enabled.
    #[allow(dead_code)]
    pub fn dbg_log_workspaces(&self) {
        if cfg!(debug_assertions) {
            for (idx, ws) in self.workspaces.lock().iter().enumerate() {
                log::debug!("workspace {idx}:");
                for c in ws.iter() {
                    log::debug!("  {}", *c);
                }
            }
        }
    }

    /// Adds a new event sink.
    /// Note: if called from an event sink that sink must return `true` from the
    ///       `accept` function to stop iteration over the now modified sink list.
    pub fn add_event_sink(&self, sink: SinkStorage) {
        self.event_sinks.borrow_mut().add(sink);
    }

    /// Removes the event sink with the given ID.
    /// Note: if called from an event sink that sink must return `true` from the
    ///       `accept` function to stop iteration over the now modified sink list.
    pub fn remove_event_sink(&self, id: SinkId) {
        self.event_sinks.borrow_mut().remove(id);
    }

    /// Removes the event sink with the given ID after handling the current
    /// signal has finished. May ONLY be called from signal handling functions.
    pub fn signal_remove_event_sink(&self, id: SinkId) {
        self.remove_sinks.borrow_mut().push(id);
    }

    /// Dispatches the given event to the event sinks.
    fn dispatch_event(&mut self, event: Event) {
        let router = self.event_sinks.get_mut();
        router.dispatch_event(&event);
        router.update();
    }

    /// Dispatches the given signal to all event sinks except the `MainEventSink`.
    /// Cannot be called from other sinks or an infinite recursion occurs.
    fn dispatch_signal(&mut self, signal: Signal) {
        self.event_sinks.get_mut().dispatch_signal(signal);
        // Signals handlers can't stop signal processing so we need deferred
        // deletion for them. Signals don't happen as often as events so the
        // performance loss if any doesn't matter.
        for remove in self.remove_sinks.borrow_mut().drain(..) {
            self.remove_event_sink(remove);
        }
        self.event_sinks.get_mut().update();
    }

    /// Stops the mainloop after the current iteration.
    pub fn quit(&self, reason: Option<String>) {
        *self.quit_reason.borrow_mut() = reason;
        self.is_running.set(false);
    }

    /// Tries to get a key binding from the config.
    /// This works regardless of toggleable modifiers set in the event.
    pub fn get_key_binding(&self, event: &KeyPressEvent) -> Option<Action> {
        let modifiers = self.modmap.borrow_mut().clean_mods(event.state());
        self.config.get_key_binding(event.detail(), modifiers)
    }

    /// Updates the modifier mapping and re-grabs all keys.
    pub fn mapping_changed(&self, mapping: Mapping) {
        match mapping {
            Mapping::Modifier => {
                {
                    let mut modmap = self.modmap.borrow_mut();
                    modmap.refresh(&self.display);
                    self.config.refresh_modifier(&modmap);
                }
                self.mapping_changed(Mapping::Keyboard);
                self.mapping_changed(Mapping::Pointer);
            }
            Mapping::Keyboard => {
                self.grab_keys().unwrap();
            }
            Mapping::Pointer => {
                self.grab_buttons().unwrap();
            }
        }
    }

    /// Returns the context map so other types can use their own context
    /// without having to implement it the window manager.
    /// Should not be used otherwise.
    pub fn context_map(&self) -> MutexGuard<ContextMap> {
        self.context_map.lock()
    }

    /// Sets the client context for the given window, after this the client
    /// can be retrieved by callding `WindowManager::win2client` with the same
    /// window.
    pub fn associate_client(&self, window: &impl Xid, client: &Arc<Client>) {
        self.context_map()
            .save(window, Context::Client(client.clone()));
    }

    /// Gets the client previously associated with the window using
    /// `WindowManager::associate_client`.
    pub fn win2client(&self, window: &impl Xid) -> Option<Arc<Client>> {
        self.context_map()
            .find(window, Context::Client)
            .map(Context::unwrap_client)
    }

    /// Sets the window kind context for the given window.
    pub fn set_window_kind(&self, window: &impl Xid, kind: WindowKind) {
        self.context_map().save(window, Context::WindowKind(kind));
    }

    /// Gets the window kind context for the given window.
    pub fn get_window_kind(&self, window: &impl Xid) -> WindowKind {
        self.context_map()
            .find(window, Context::WindowKind)
            .map(Context::unwrap_window_kind)
            .unwrap_or(WindowKind::MetaOrUnmanaged)
    }

    /// Checks if source window of the given event has the given window kind.
    /// Note that the source is not available for all events (see [`x_event_source`]),
    /// in which case `false` is always returned.
    pub fn source_kind_matches(&self, event: &Event, kind: WindowKind) -> bool {
        if let Some(source) = x_event_source(event) {
            discriminant(&self.get_window_kind(&source)) == discriminant(&kind)
        } else {
            false
        }
    }

    /// Removes all possible context values for the given window.
    pub fn remove_all_contexts(&self, window: &impl Xid) {
        self.context_map().delete_all(window);
    }

    /// Adds the given window to the list of unmanaged/meta windows.
    pub fn add_unmanaged(&self, window: Window) {
        self.unmanaged.borrow_mut().push(window);
    }

    /// If the given window is an unamanged/meta window, remove it and return
    /// `true`. Returns `false` otherwise.
    pub fn maybe_remove_unmanaged(&self, window: XcbWindow) -> bool {
        let mut unmanaged = self.unmanaged.borrow_mut();
        if let Some(idx) = unmanaged.iter().position(|w| w.handle() == window) {
            unmanaged.remove(idx);
            true
        } else {
            false
        }
    }

    /// Refreshes the `_NET_CLIENT_LIST` property.
    pub fn update_client_list(&self) {
        let display = &self.display;
        self.root
            .delete_property(display, display.atoms.net_client_list);
        for workspace in self.workspaces.lock().iter() {
            for client in workspace.iter() {
                self.root.append_property(
                    display,
                    display.atoms.net_client_list,
                    PropertyValue::Window(client.handle()),
                );
            }
        }
    }

    /// Updates the geometry of all fullscreen windows.
    pub fn update_fullscreen_windows(&self) {
        let monitors = monitors();
        let workspaces = self.workspaces.lock();
        for workspace in workspaces.iter() {
            for client in workspace.iter() {
                if client.state().is_fullscreen() {
                    let mon = monitors.containing(client);
                    client.window().move_and_resize(*mon.geometry());
                }
            }
        }
    }

    /// Shows a desktop notification.
    pub fn notify(&self, summary: &str, body: &str, icon: &str, timeout: i32) {
        let props = NotificationProps {
            app_name: "",
            app_icon: icon,
            summary,
            body,
            actions: &[],
            hints: &HashMap::new(),
        };
        let mut manager = self.notification_manager.lock();
        let id = manager.get_id(0);
        manager.new_notification(id, &props);
        if let Some(timeout) = match timeout.cmp(&0) {
            Ordering::Less => Some(None),
            Ordering::Greater => Some(Some(timeout)),
            Ordering::Equal => None,
        } {
            manager.close_after(id, timeout, self.notification_manager.clone());
        }
    }

    /// Returns `true` is an audio api is available.
    pub fn has_audio_api(&self) -> bool {
        self.audio_api.is_some()
    }

    /// Returns a mutex guard for the audio api, if there is one.
    pub fn audio_api(&self) -> Option<MutexGuard<Box<dyn AudioAPI>>> {
        self.audio_api.as_ref().map(|mtx| mtx.lock())
    }

    /// Assumes there is an audio api and returns a mutex guard for it.
    pub fn audio_api_unchecked(&self) -> MutexGuard<Box<dyn AudioAPI>> {
        unsafe { self.audio_api.as_ref().unwrap_unchecked() }.lock()
    }

    fn grab_keys(&self) -> AnyResult<()> {
        self.display.ungrab_key(GRAB_ANY, ModMask::ANY);
        for key in self.config.iter_keys() {
            self.display.grab_key(key.code(), key.modifiers());
        }
        self.display.flush();
        Ok(())
    }

    fn grab_buttons(&self) -> AnyResult<()> {
        self.display.ungrab_button(ButtonIndex::Any, ModMask::ANY);
        self.display
            .grab_button(ButtonIndex::N1, self.config.modifier());
        self.display
            .grab_button(ButtonIndex::N1, self.config.modifier() | ModMask::SHIFT);
        self.display
            .grab_button(ButtonIndex::N3, self.config.modifier());
        self.display.flush();
        Ok(())
    }

    fn select_root_events(&self) {
        #[rustfmt::skip]
        self.root.change_event_mask(
            EventMask::SUBSTRUCTURE_REDIRECT
          | EventMask::SUBSTRUCTURE_NOTIFY
          | EventMask::BUTTON_PRESS
          | EventMask::BUTTON_RELEASE
          | EventMask::POINTER_MOTION
          | EventMask::STRUCTURE_NOTIFY
          | EventMask::PROPERTY_CHANGE
        );
    }

    fn run_autostartrc(&self) {
        let path = paths::autostart_path();
        if std::fs::metadata(&path).is_ok() {
            run_and_await(&["bash", path.as_str()]).or_fatal(&self.display);
        }
    }

    fn init(&self, this: &Arc<Self>) -> AnyResult<()> {
        log::trace!("Creating workspaces");
        let mut workspaces = self.workspaces.lock();
        for i in 0..self.config.layout.workspaces {
            workspaces.push(Workspace::new(i, self));
        }
        drop(workspaces);
        log::trace!("Setting root properties");
        self.root.setup();
        self.root.change_attributes(|attributes| {
            attributes.cursor(self.cursors.normal);
        });
        log::trace!("Grabbing input");
        self.grab_keys()?;
        self.grab_buttons()?;
        self.select_root_events();
        log::trace!("Running autostart script");
        self.run_autostartrc();
        log::trace!("Caching startup wm classes");
        set_startup_wm_classes();
        {
            let mut e = this.event_sinks.borrow_mut();
            log::trace!("bar: creating bar");
            if self.config.bar.enable {
                e.add(SinkStorage::Unique(Box::new(bar::create(this.clone()))));
            }
            log::trace!("dock: creating dock");
            if self.config.dock.enable {
                e.add(SinkStorage::Mutex(Dock::new(this)));
            }
            e.update();
        }
        Ok(())
    }

    fn run(&mut self) -> AnyResult<()> {
        self.display.flush();
        while self.is_running.get() {
            match self.display.next_event() {
                Ok(event) => self.dispatch_event(event),
                Err(error) => {
                    log::error!("X error: {error:?}");
                }
            }
            while let Ok(sig) = self.signal_receiver.try_recv() {
                self.dispatch_signal(sig);
            }
        }
        Ok(())
    }

    fn cleanup(&mut self) -> AnyResult<()> {
        log::trace!("Closing clients");
        {
            let mut workspaces = self.workspaces.lock();
            for workspace in workspaces.iter() {
                for client in workspace.iter() {
                    close_client(client);
                }
            }
            workspaces.clear();
        }
        log::trace!("Closing meta windows");
        for window in self.unmanaged.borrow().iter() {
            self.remove_all_contexts(window);
            close_window(window);
        }
        self.unmanaged.borrow_mut().clear();
        log::trace!("Sending quit signal");
        self.dispatch_signal(Signal::Quit);
        self.event_sinks.borrow_mut().clear();
        log::trace!("Removing D-Bus interfaces");
        self.session_manager.lock().unregister();
        self.notification_manager.lock().unregister();
        log::trace!("Cleaning root properties");
        self.root.clean();
        self.drawing_context.lock().destroy();
        Ok(())
    }

    pub fn main(display: Arc<Display>) -> AnyResult<()> {
        let arc = Self::new(display);
        let this: &'static mut _ = unsafe {
            let ptr = arc.as_ptr() as *const Self as *mut Self;
            &mut *ptr
        };
        log::trace!("================================================================");
        log::trace!("Init");
        log::trace!("================================================================");
        this.init(&arc)?;
        log::trace!("================================================================");
        log::trace!("Run");
        log::trace!("================================================================");
        this.run()?;
        log::trace!("================================================================");
        log::trace!("Cleanup");
        log::trace!("================================================================");
        this.cleanup()?;
        // Some diagnostics
        if cfg!(debug_assertions) {
            let strong_count = Arc::strong_count(&arc);
            if strong_count > 1 {
                log::warn!("{strong_count} references to window manager at shutdown (should be 1)");
            }
            if !this.context_map().is_empty() {
                log::warn!(
                    "Context map not empty at shutdown: {:#?}",
                    *this.context_map.lock()
                );
            }
        }
        // Quit reasons
        if let Some(quit_reason) = this.quit_reason.borrow().as_ref() {
            match quit_reason.as_str() {
                "logout" => {
                    log::info!("logging out");
                    log_error!(platform::logout());
                }
                "restart" => {
                    log::info!("restarting");
                    log_error!(system_shutdown::reboot());
                }
                "shutdown" => {
                    log::info!("shutting down");
                    log_error!(system_shutdown::shutdown());
                }
                other => log::warn!("invalid quit reason: '{other}'"),
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for WindowManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WindowManager")
    }
}
