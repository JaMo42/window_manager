use crate::{
    cfg::{parse::Value, scanner::Scanner},
    color::{BorderColor, Color},
    AnyResult,
};
use std::collections::BTreeMap;

/// Configuration kind for a color.
#[derive(Debug, Clone)]
pub enum ColorConfig {
    Default,
    Value(Color),
    Link(String),
}

/// Configuration for a colorscheme.
/// The contained vector always the same size as the color scheme.
#[derive(Debug)]
pub struct ColorSchemeConfig {
    pub cfg: Vec<ColorConfig>,
}

impl Default for ColorSchemeConfig {
    fn default() -> Self {
        ColorSchemeConfig {
            cfg: vec![ColorConfig::Default; COLOR_COUNT],
        }
    }
}

impl ColorSchemeConfig {
    pub fn set(&mut self, elem: &str, cfg: ColorConfig) -> Result<(), String> {
        self.cfg[color_index(elem)?] = cfg;
        Ok(())
    }
}

macro_rules! color_scheme {
    {
        $(
            $field:ident => ($config_name:expr, $default:expr),
        )*
    } => {
        pub struct ColorScheme {
            $(pub $field: Color,)*
        }
        const COLOR_COUNT: usize
            = std::mem::size_of::<ColorScheme>() / std::mem::size_of::<Color>();
        const COLOR_NAMES: [&str; COLOR_COUNT] = [
            $($config_name,)*
        ];
        const DEFAULT_CONFIG: [&str; COLOR_COUNT] = [
            $($default,)*
        ];
    }
}

color_scheme! {
    // Only used to set defaults of other elements:
    text => ("text", "#EEEEEE"),
    background => ("background", "#111111"),
    // Actual elements:
    focused => ("window.focused", "#EEEEEE"),
    focused_text => ("window.focused_text", "#111111"),
    normal => ("window.normal", "#111111"),
    normal_text => ("window.normal_text", "#EEEEEE"),
    selected => ("window.selected", "#777777"),
    selected_text => ("window.selected_text", "#111111"),
    urgent => ("window.urgent", "#CC1111"),
    urgent_text => ("window.urgent_text", "#111111"),
    close_button => ("window.buttons.close", "#444444"),
    close_button_hovered => ("window.buttons.close_hovered", "#CC0000"),
    maximize_button => ("window.buttons.maximize", "window.buttons.close"),
    maximize_button_hovered => ("window.buttons.maximize_hovered", "#00CC00"),
    minimize_button => ("window.buttons.minimize", "window.buttons.close"),
    minimize_button_hovered => ("window.buttons.minimize_hovered", "#CCCC00"),
    background_color => ("misc.background", "#000000"),
    bar_background => ("bar.background", "#111111"),
    bar_text => ("bar.text", "#EEEEEE"),
    bar_workspace => ("bar.workspace", "background"),
    bar_workspace_text => ("bar.workspace_text", "bar.text"),
    bar_active_workspace => ("bar.active_workspace", "window.focused"),
    bar_active_workspace_text => ("bar.active_workspace_text", "window.focused_text"),
    bar_urgent_workspace => ("bar.urgent_workspace", "window.urgent"),
    bar_urgent_workspace_text => ("bar.urgent_workspace_text", "window.urgent_text"),
    notification_background => ("notifications.background", "background"),
    notification_text => ("notifications.text", "text"),
    tooltip_background => ("tooltip.background", "background"),
    tooltip_text => ("tooltip.text", "text"),
    dock_background => ("dock.background", "background"),
    dock_hovered => ("dock.hovered", "window.focused"),
    dock_urgent => ("dock.urgent", "window.urgent"),
    dock_indicator => ("dock.indicator", "text"),
    context_menu_background => ("context_menu.background", "background"),
    context_menu_text => ("context_menu.text", "text"),
    context_menu_divider => ("context_menu.divider", "text"),
}

impl std::fmt::Debug for ColorScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ColorScheme(...)")
    }
}

impl std::ops::Index<usize> for ColorScheme {
    type Output = Color;

    fn index(&self, index: usize) -> &Color {
        let p = self as *const ColorScheme as *const Color;
        unsafe { &*p.add(index) }
    }
}

impl std::ops::IndexMut<usize> for ColorScheme {
    fn index_mut(&mut self, index: usize) -> &mut Color {
        let p = self as *mut ColorScheme as *mut Color;
        unsafe { &mut *p.add(index) }
    }
}

impl ColorScheme {
    fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }

    pub fn new(cfg: &ColorSchemeConfig, defs: &BTreeMap<String, Color>) -> AnyResult<Self> {
        let mut result = Self::zeroed();
        let mut has_value: [bool; COLOR_COUNT] = [false; COLOR_COUNT];
        let mut links = Vec::<(usize, usize)>::with_capacity(COLOR_COUNT / 2);
        for i in 0..COLOR_COUNT {
            match &cfg.cfg[i] {
                ColorConfig::Default => {
                    if DEFAULT_CONFIG[i].starts_with('#') {
                        result[i] = Color::parse(&mut Scanner::new(DEFAULT_CONFIG[i]))
                            .expect("Invalid color in builtin scheme");
                        has_value[i] = true;
                    } else {
                        links.push((i, color_index(DEFAULT_CONFIG[i])?));
                    }
                }
                ColorConfig::Value(value) => {
                    result[i] = *value;
                    has_value[i] = true;
                }
                ColorConfig::Link(target) => {
                    if let Some(def) = defs.get(target) {
                        result[i] = *def;
                        has_value[i] = true;
                    } else {
                        links.push((i, color_index(target.as_str())?));
                    }
                }
            }
        }

        let mut did_change = true;
        while did_change && !links.is_empty() {
            did_change = false;
            for i in (0..links.len()).rev() {
                if has_value[links[i].1] {
                    result[links[i].0] = result[links[i].1];
                    has_value[links[i].0] = true;
                    links.remove(i);
                    did_change = true;
                }
            }
        }

        if !links.is_empty() {
            Err(format!("Unresolved links in color scheme: {:#?}", links))?
        } else {
            Ok(result)
        }
    }

    pub fn focused_border(&self) -> BorderColor {
        BorderColor::new(0, true, self.focused, self.focused_text)
    }

    pub fn normal_border(&self) -> BorderColor {
        BorderColor::new(1, false, self.normal, self.normal_text)
    }

    pub fn selected_border(&self) -> BorderColor {
        BorderColor::new(2, true, self.selected, self.selected_text)
    }

    pub fn urgent_border(&self) -> BorderColor {
        BorderColor::new(3, false, self.urgent, self.urgent_text)
    }

    pub fn bar(&self) -> BorderColor {
        BorderColor::new(
            crate::bar::COLOR_KIND,
            false,
            self.bar_background,
            self.bar_text,
        )
    }
}

fn color_index(name: &str) -> Result<usize, String> {
    for (i, color) in COLOR_NAMES.iter().enumerate() {
        if *color == name {
            return Ok(i);
        }
    }
    Err(format!("Invalid color name: {}", name))
}
