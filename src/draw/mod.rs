mod context;
mod resource;
mod shape;
mod text;

pub use context::{create_xcb_surface, DrawingContext};
pub use resource::{load_builtin_svg, BuiltinResources, Svg};
pub use shape::{ColorKind, GradientSpec};
pub use text::{Alignment, TextBuilder};
