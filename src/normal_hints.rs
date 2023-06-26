use crate::{rectangle::Rectangle, x::Window};
use x11::xlib::{PAspect, PMaxSize, PMinSize, PResizeInc};

#[derive(Copy, Clone, Debug, Default)]
pub struct NormalHints {
    min_size: Option<(u16, u16)>,
    max_size: Option<(u16, u16)>,
    resize_inc: Option<(i16, i16)>,
    aspect_ratio: Option<(f64, f64)>,
}

impl NormalHints {
    /// Get the normal hints for the given window.
    pub fn get(window: &Window) -> Option<Self> {
        let hints = window.display().get_wm_normal_hints(window)?;
        let mut result = Self::default();
        if hints.flags & PMinSize == PMinSize {
            result.min_size = Some((
                hints.min_width.min(u16::MAX as i32) as u16,
                hints.min_height.min(u16::MAX as i32) as u16,
            ));
        }
        if hints.flags & PMaxSize == PMaxSize {
            result.max_size = Some((
                hints.max_width.min(u16::MAX as i32) as u16,
                hints.max_height.min(u16::MAX as i32) as u16,
            ));
        }
        if hints.flags & PResizeInc == PResizeInc {
            result.resize_inc = Some((
                hints.width_inc.min(u16::MAX as i32) as i16,
                hints.height_inc.min(u16::MAX as i32) as i16
            ));
        }
        if hints.flags & PAspect == PAspect {
            result.aspect_ratio = Some((
                hints.min_aspect.x as f64 / hints.min_aspect.y as f64,
                hints.max_aspect.x as f64 / hints.max_aspect.y as f64,
            ));
        }
        Some(result)
    }

    /// Applies the hints to the given rectangle.
    /// If `keep_height` is `true` the width will be changed instead of the
    /// height when adjusting the aspect ratio, it has no effect on the other
    /// size limits.
    /// This only applies size constraints, not resize increments.
    pub fn constrain(&self, rect: &Rectangle, keep_height: bool) -> Rectangle {
        let mut result = *rect;
        if let Some((minw, minh)) = self.min_size {
            result.width = u16::max(result.width, minw);
            result.height = u16::max(result.height, minh);
        }
        if let Some((maxw, maxh)) = self.max_size {
            result.width = u16::min(result.width, maxw);
            result.height = u16::min(result.height, maxh);
        }
        if let Some((min_aspect, max_aspect)) = self.aspect_ratio {
            let in_ratio = rect.width as f64 / rect.height as f64;
            let mut correct = None;
            if in_ratio < min_aspect {
                correct = Some(min_aspect);
            } else if in_ratio > max_aspect {
                correct = Some(max_aspect);
            }
            if let Some(ratio) = correct {
                if keep_height {
                    result.width = (result.height as f64 / (1.0 / ratio)).round() as u16;
                } else {
                    result.height = (result.width as f64 / ratio).round() as u16;
                }
            }
        }
        result
    }

    pub fn resize_inc(&self) -> Option<(i16, i16)> {
        self.resize_inc
    }
}
