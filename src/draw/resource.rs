use super::DrawingContext;
use crate::{desktop_entry::DesktopEntry, rectangle::Rectangle, AnyResult};
use cairo::Pattern;
use gio::{Cancellable, File, MemoryInputStream};
use glib::Bytes;
use include_dir::{include_dir, Dir};
use librsvg::{CairoRenderer, Loader, SvgHandle};
use std::{cell::RefCell, rc::Rc};

const RESOURCE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/res");

macro_rules! _load {
    // Svg using the same name as its identifier
    ($field_name:ident, svg) => {
        load_builtin_svg(stringify!($field_name))
    };
    // Svg with name
    ($field_name:ident, svg $name:expr) => {
        load_builtin_svg(stringify!($name))
    };
}

macro_rules! builtin_resources {
    {
        $(
            $name:ident: $type:ty => $kind:ident $($more:expr)?,
        )*
    } => {
        pub struct BuiltinResources {
            $($name: Rc<$type>,)*
        }
        impl BuiltinResources {
            pub fn load_all() -> Self {
                Self {
                    $(
                        $name: Rc::new(_load!($name, $kind $($more)?)),
                    )*
                }
            }
            $(
                pub fn $name(&self) -> &Rc<$type> {
                    &self.$name
                }
            )*
        }
    }
}

builtin_resources! {
    calendar: Svg => svg,
    close_button: Svg => svg,
    maximize_button: Svg => svg,
    minimize_button: Svg => svg,
    power: Svg => svg,
    volume: Svg => svg,
    volume_muted: Svg => svg,
    minus: Svg => svg,
    plus: Svg => svg,
}

pub struct Svg {
    renderer: CairoRenderer<'static>,
    _handle: Box<SvgHandle>,
    pattern: RefCell<Option<(Rectangle, Pattern)>>,
}

// `Pattern` does not implement `Send` since it holds a raw pointer.
unsafe impl Send for Svg {}

impl Svg {
    fn new(handle: Box<SvgHandle>) -> Self {
        // We can just give the renderer a static reference since the lifetime of
        // the renderer and the handle are both tied to the `Svg` object and the
        // handles is boxed so it doesn't move in memory.
        let static_handle: &'static _ = unsafe { &*(handle.as_ref() as *const SvgHandle) };
        let renderer = CairoRenderer::new(static_handle);
        Self {
            renderer,
            _handle: handle,
            pattern: RefCell::new(None),
        }
    }

    /// Loads an SVG from a file.
    pub fn try_load(pathname: &str) -> AnyResult<Self> {
        let loader = Loader::new();
        let handle = Box::new(loader.read_path(pathname)?);
        Ok(Self::new(handle))
    }

    /// Loads an SVG from a byte array, the given data is assumed to be valid.
    pub fn from_bytes(bytes: &'static [u8]) -> Self {
        let bytes = Bytes::from_static(bytes);
        let stream = MemoryInputStream::from_bytes(&bytes);
        let handle = Box::new(
            Loader::new()
                .read_stream(&stream, None::<&File>, None::<&Cancellable>)
                .unwrap(),
        );
        Self::new(handle)
    }

    pub fn renderer(&self) -> &CairoRenderer {
        &self.renderer
    }

    pub fn get_pattern(&self, rect: Rectangle, dc: &DrawingContext) -> AnyResult<Pattern> {
        // The cairo pattern uses internal reference counting so cloning here is cheap.
        if let Some((last_rect, pattern)) = self.pattern.borrow().clone() {
            if rect == last_rect {
                return Ok(pattern);
            }
        }
        let context = dc.cairo();
        context.save()?;
        context.push_group();
        dc.draw_svg(self, rect);
        let pattern = context.pop_group()?;
        *self.pattern.borrow_mut() = Some((rect, pattern.clone()));
        context.restore()?;
        Ok(pattern)
    }
}

pub fn load_builtin_svg(name: &str) -> Svg {
    let file = RESOURCE_DIR
        .get_file(&format!("{name}.svg"))
        .unwrap_or_else(|| panic!("missing builtin resource: {name}"));
    Svg::from_bytes(file.contents())
}

pub fn load_icon(name: &str, theme: &str) -> Option<AnyResult<Svg>> {
    const DIRS: [&str; 10] = [
        "apps",
        "actions",
        "categories",
        "status",
        "devices",
        "emblems",
        "emotes",
        "intl",
        "mimetypes",
        "places",
    ];
    if name.is_empty() {
        return None;
    }
    if name.starts_with('/') {
        // TODO: should maybe return `None` if the path does not exist.
        return Some(Svg::try_load(name));
    }
    for d in DIRS {
        let pathname = format!("{}/48x48/{}/{}.svg", theme, d, name);
        if std::fs::metadata(&pathname).is_ok() {
            return Some(Svg::try_load(&pathname));
        }
    }
    log::trace!("No icon found for name '{name}'");
    None
}

pub fn load_app_icon(application_name: &str, theme: &str) -> Option<Svg> {
    if application_name.is_empty() {
        return None;
    }
    let desktop_entry = DesktopEntry::new(application_name)?;
    let name = desktop_entry.icon?;
    let icon_path = if name.starts_with('/') {
        name
    } else {
        format!("{}/48x48/apps/{}.svg", theme, name)
    };
    Svg::try_load(&icon_path).ok()
}
