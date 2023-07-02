use crate::{
    action,
    cfg::{gen::parsed_config, parse::Parser},
    client::Client,
    color_scheme::ColorScheme,
    config_types::ColorSchemeParser,
    draw::{Alignment, DrawingContext},
    error::OrFatal,
    layout::{lerp, ClientLayout, LayoutClass},
    monitors::{monitors_mut, WindowAreaPadding},
    paths,
    window_manager::WindowManager,
    x::{string_to_keysym, Display, ModifierMapping},
    AnyResult,
};
use pango::FontDescription;
use std::{
    cell::RefCell,
    collections::{hash_map, HashMap},
    rc::Rc,
    sync::Arc,
};
use x11::keysym;
use xcb::x::{KeyButMask, Keycode, ModMask};

#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub struct Key {
    modifiers: ModMask,
    code: Keycode,
}

impl Key {
    pub fn from_str(display: &Display, s: &str, modifiers: ModMask) -> Option<Self> {
        string_to_keysym(s).map(|sym| Self::from_sym(display, sym as u32, modifiers))
    }

    pub fn from_sym(display: &Display, sym: u32, modifiers: ModMask) -> Self {
        Self {
            modifiers,
            code: display.keysym_to_keycode(sym),
        }
    }

    pub fn modifiers(&self) -> ModMask {
        self.modifiers
    }

    pub fn code(&self) -> u8 {
        self.code
    }
}

fn str2mod(s: &str, user_mod: ModMask, modmap: &ModifierMapping) -> ModMask {
    match s.to_ascii_lowercase().as_str() {
        "win" => modmap.win(),
        "shift" => modmap.shift(),
        "alt" => modmap.alt(),
        "ctrl" => modmap.control(),
        "mod" => user_mod,
        _ => ModMask::empty(),
    }
}

/// Parses a `+` separated sequence of modifiers. The user mod is defaulted to
/// to super/win key.
fn modifiers_from_string(s: &str, modmap: &ModifierMapping) -> ModMask {
    let mut mods = ModMask::empty();
    for mod_str in s.split('+') {
        mods |= str2mod(mod_str, modmap.win(), modmap);
    }
    mods
}

fn mods_and_key_from_string<'a>(
    s: &'a str,
    user_mod: ModMask,
    modmap: &ModifierMapping,
) -> (ModMask, &'a str) {
    let mut mods = ModMask::empty();
    let mut key = "";
    let mut it = s.split('+').peekable();
    while let Some(i) = it.next() {
        if it.peek().is_some() {
            mods |= str2mod(i, user_mod, modmap);
        } else {
            key = i;
        }
    }
    (mods, key)
}

#[derive(Copy, Clone)]
pub struct WorkspaceAction(
    /// function: fn(window_manager, workspace_index, maybe_client)
    pub fn(&WindowManager, usize, Option<&Client>),
    /// `workspace_index` argument for the function
    pub usize,
    /// whether this action requires a focused client
    pub bool,
);

#[derive(Clone)]
pub enum Action {
    Client(fn(&Client)),
    Workspace(WorkspaceAction),
    Launch(Vec<String>),
    Generic(fn(&WindowManager)),
}

#[derive(Default)]
pub struct KeyBindings {
    pairs: Vec<(String, Action)>,
}

impl KeyBindings {
    pub fn push(&mut self, mods_and_key: String, action: Action) {
        self.pairs.push((mods_and_key, action));
    }

    fn into_map(
        self,
        display: &Display,
        user_mod: ModMask,
        modmap: &ModifierMapping,
    ) -> HashMap<Key, Action> {
        let mut map = HashMap::new();
        for (mods_and_key, action) in self.pairs.into_iter() {
            let (mods, key) = mods_and_key_from_string(&mods_and_key, user_mod, modmap);
            // Validity of key was already checked during parsing so unwrap is
            // safe here.
            let key = Key::from_str(display, key, mods).unwrap();
            map.insert(key, action);
        }
        map
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Size {
    /// Direct pixel count
    Pixels(u16),
    /// Millimeters.
    Physical(f64),
    /// Percentage of a superior element.
    Percent(f64),
    /// Size relative to some font size.
    PercentOfFont(f64),
}

impl Size {
    /// Creates a new pixel size value.
    pub fn new(pixels: u16) -> Self {
        Self::Pixels(pixels)
    }

    /// Constructs a new size value based on the given suffix.
    pub fn new_with_suffix(num: f64, suffix: &str) -> Self {
        match suffix {
            "" | "px" => Self::Pixels(num.round() as u16),
            "mm" => Self::Physical(num),
            "cm" => Self::Physical(num * 10.0),
            "%" => Self::Percent(num.clamp(0.0, 100.0) / 100.0),
            "em" => Self::PercentOfFont(num.max(1.0)),
            x => panic!("invalid suffix for size: {x}"),
        }
    }

    /// Tries to resolve the size to a pixel value.
    pub fn resolve(
        &self,
        dpmm: Option<f64>,
        relative_to: Option<u16>,
        font_height: Option<u16>,
    ) -> u16 {
        match *self {
            Self::Pixels(pixels) => pixels,
            Self::Physical(mm) => (dpmm.unwrap() * mm).round() as u16,
            Self::Percent(p) => (relative_to.unwrap() as f64 * p).round() as u16,
            Self::PercentOfFont(p) => (font_height.unwrap() as f64 * p).round() as u16,
        }
    }
}

fn parse_color_scheme(display: &Display, name: &str) -> AnyResult<ColorScheme> {
    log::trace!("Loading color scheme");
    let pathname = format!("{}/{}.ini", paths::colors_dir(), name);
    let mut parser = Parser::new(display, &pathname)?;
    let mut scheme_parser = ColorSchemeParser::default();
    parser.parse(&mut scheme_parser);
    scheme_parser.finish()
}

/// Tries to find the full path to the given icon theme name.
fn find_icon_theme(name: &str) -> AnyResult<String> {
    let home = std::env::var("HOME")?;
    let directories = [
        "/usr/share/icons".to_string(),
        format!("{}/{}", home, ".local/share/icons"),
        format!("{}/{}", home, ".icons"),
    ];
    log::trace!("Looking for icon theme");
    for d in directories {
        let path = format!("{}/{}", d, name);
        log::trace!(" - {}", path);
        if std::fs::metadata(&path).is_ok() {
            log::trace!(" -> found");
            return Ok(path);
        }
    }
    log::trace!(" -> none found");
    Err(format!("Icon theme not found: {}", name))?
}

parsed_config! {
    sections => {
        General {
            meta_window_classes: Vec<String> = "[]",
            default_notification_timeout: u64 = "6000",
            double_click_time: u32 = "500",
            grid_resize: bool = "false",
            grid_resize_grid_size: (u16, u16) = "[16, 9]",
            grid_resize_live: bool = "false",
            scale_base_fonts: bool = "true",
        }
        Layout {
            workspaces: usize = "1",
            gaps: Size = "0",
            padding: WindowAreaPadding = "[0, 0, 0, 0]",
            secondary_padding: WindowAreaPadding = "[0, 0, 0, 0]",
            smart_window_placement: bool = "true",
            smart_window_placement_max: usize = "0",
        }
        Window {
            border: Size = "1mm",
            title_font: FontDescription = "'sans'",
            title_font_size: u16 = "14",
            title_font_scaling_percent: u16 = "100",
            title_bar_height: Size = "1.1em",
            title_alignment: Alignment = "left",
            left_buttons: Vec<String> = "[]",
            right_buttons: Vec<String> = "['close']",
            icon_size: Size = "80%",
            button_icon_size: Size = "75%",
            circle_buttons: bool = "false",
            extend_frame: Size = "1mm",
        }
        Theme {
            colors: String = "'default'",
            icons: String = "'Papirus'",
        }
        Keys {
            modifier: String = "'Super'",
        }
        Bar {
            enable: bool = "true",
            height: Size = "1.1em",
            font: FontDescription = "'sans 14'",
            time_format: String = "'%a %b %e %H:%M %Y'",
            localized_time: bool = "true",
            power_supply: String = "'BAT0'",
            update_interval: u64 = "10000",
            volume_mixer_title_width: Size = "15%",
            volume_mixer_grouping: String = "'name'",
        }
        Dock {
            enable: bool = "true",
            height: Size = "10%",
            offset: Size = "0",
            pinned: Vec<String> = "[]",
            focused_client_on_top: bool = "false",
            focus_urgent: bool = "false",
            item_size: Size = "80%",
            icon_size: Size = "85%",
            context_show_workspaces: bool = "true",
            auto_indicator_colors: bool = "true",
        }
        SplitHandles {
            size: Size = "2mm",
            vertical_sticky: Vec<u16> = "[50]",
            horizontal_sticky: Vec<u16> = "[50]",
            min_split_size: u16 = "10",
        }
    }
    config => {
        general: General => "general",
        layout: Layout => "layout",
        window: Window => "window",
        theme: Theme => "theme",
        keys: Keys => "keys",
        key_bindings: KeyBindings => "keys.bindings",
        bar: Bar => "bar",
        dock: Dock => "dock",
        split_handles: SplitHandles => "split_handles",
    }
}

pub struct Config {
    key_binds: HashMap<Key, Action>,
    modifier_str: String,
    modifier: RefCell<ModMask>,
    client_layout: Rc<RefCell<LayoutClass<ClientLayout>>>,
    pub general: General,
    pub layout: Layout,
    pub window: Window,
    pub bar: Bar,
    pub dock: Dock,
    pub split_handles: SplitHandles,
    pub colors: ColorScheme,
    pub icon_theme: String,
}

unsafe impl Sync for Config {}

impl Config {
    pub fn try_load(display: &Arc<Display>, dc: &DrawingContext) -> AnyResult<Self> {
        let mut parsed = ParsedConfig::default();
        log::trace!("Loading configuration");
        Parser::new(display, paths::config_path().as_str())?.parse(&mut parsed);
        let mut modmap = ModifierMapping::default();
        modmap.refresh(display);
        let user_mod = modifiers_from_string(&parsed.keys.modifier, &modmap);
        let mut this = Self {
            key_binds: parsed.key_bindings.into_map(display, user_mod, &modmap),
            modifier_str: parsed.keys.modifier,
            modifier: RefCell::new(user_mod),
            client_layout: Rc::new(RefCell::new(LayoutClass::default())),
            general: parsed.general,
            layout: parsed.layout,
            window: parsed.window,
            bar: parsed.bar,
            dock: parsed.dock,
            split_handles: parsed.split_handles,
            colors: parse_color_scheme(display, &parsed.theme.colors)?,
            icon_theme: find_icon_theme(&parsed.theme.icons)?,
        };
        for ws_idx in 0..this.layout.workspaces {
            let sym = keysym::XK_1 + ws_idx as u32;
            this.key_binds.insert(
                Key::from_sym(display, sym, user_mod),
                Action::Workspace(WorkspaceAction(action::select_workspace, ws_idx, false)),
            );
            this.key_binds.insert(
                Key::from_sym(display, sym, user_mod | modmap.shift()),
                Action::Workspace(WorkspaceAction(action::move_to_workspace, ws_idx, true)),
            );
        }
        this.key_binds.insert(
            Key::from_sym(display, keysym::XK_Tab, modmap.alt()),
            Action::Generic(action::switch_window),
        );
        this.recompute_layouts(dc);
        monitors_mut().set_window_areas(&this.layout.padding, &this.layout.secondary_padding);
        this.general
            .meta_window_classes
            .retain(|class| !class.is_empty());
        // todo: verify all relative size values are valid.
        Ok(this)
    }

    pub fn load(display: &Arc<Display>, dc: &DrawingContext) -> Self {
        Self::try_load(display, dc).unwrap_or_fatal(display)
    }

    pub fn modifier(&self) -> ModMask {
        *self.modifier.borrow()
    }

    pub fn refresh_modifier(&self, modmap: &ModifierMapping) {
        *self.modifier.borrow_mut() = str2mod(&self.modifier_str, modmap.win(), modmap);
    }

    pub fn get_key_binding(&self, code: u8, modifiers: KeyButMask) -> Option<Action> {
        let mod_bits = modifiers.bits() & ModMask::all().bits();
        let modifiers = unsafe { ModMask::from_bits_unchecked(mod_bits) };
        self.key_binds.get(&Key { modifiers, code }).cloned()
    }

    pub fn iter_keys(&self) -> hash_map::Keys<Key, Action> {
        self.key_binds.keys()
    }

    pub fn recompute_layouts(&self, dc: &DrawingContext) {
        self.client_layout.replace(LayoutClass::new(self, dc));
    }

    pub fn scale_fonts(&self, factor: f64) {
        fn scale_font(font: &mut FontDescription, factor: f64) {
            font.set_size((font.size() as f64 * factor).round() as i32)
        }
        let percent = self.window.title_font_scaling_percent as f64 / 100.0;
        let factor = lerp(1.0, factor, percent);
        log::trace!("Scaling fonts by {}%", (factor * 100.0).round() as u16);
        // These were originally not supposed to be mutable and I don't want
        // to put them all in cell types. This should be fine.
        #[allow(clippy::cast_ref_to_mut)]
        let this = unsafe { &mut *(self as *const Self as *mut Self) };
        scale_font(&mut this.bar.font, factor);
        if this.window.title_font_size > 0 {
            this.window.title_font_size =
                (this.window.title_font_size as f64 * factor).round() as u16;
        } else {
            scale_font(&mut this.window.title_font, factor);
        }
    }

    pub fn client_layout(&self) -> Rc<RefCell<LayoutClass<ClientLayout>>> {
        self.client_layout.clone()
    }
}
