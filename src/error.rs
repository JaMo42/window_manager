use crate::color::Color;
use crate::core::*;
use crate::draw::{Alignment, Drawing_Context};
use pango::FontDescription;
use x11::xlib::*;

/// Shows the given text and quits after any key is pressed.
pub unsafe fn fatal_error (text: &str) -> ! {
  let font = FontDescription::from_string ("sans 24");
  let background_color = Color::from_rgb (0.12, 0.12, 0.12);
  let text_color = Color::from_rgb (0.91, 0.92, 0.92);
  let border = 50;
  let height = screen_size.h - 2 * border as u32;

  let mut my_draw = Drawing_Context::new ();
  let mut attributes: XSetWindowAttributes = uninitialized! ();
  attributes.event_mask = KeyPressMask;
  let window = XCreateWindow (
    display,
    root,
    0,
    0,
    screen_size.w,
    screen_size.h,
    0,
    CopyFromParent,
    CopyFromParent as u32,
    CopyFromParent as *mut Visual,
    CWEventMask,
    &mut attributes,
  );
  XMapRaised (display, window);

  my_draw
    .rect (0, 0, screen_size.w, screen_size.h)
    .color (background_color)
    .draw ();
  my_draw.select_font (&font);
  my_draw.text_color (text_color);
  my_draw.text (text).at (border, border).draw ();
  my_draw
    .text ("Press any key to quit")
    .at (border, border)
    .align_vertically (Alignment::Bottom, height as i32)
    .draw ();
  my_draw.render (window, 0, 0, screen_size.w, screen_size.h);

  let mut event: XEvent = uninitialized! ();
  running = true;
  XSync (display, X_FALSE);
  while running {
    XNextEvent (display, &mut event);
    if event.type_ == KeyPress {
      running = false;
    }
  }
  my_draw.destroy ();
  XDestroyWindow (display, window);
  std::process::exit (1);
}

pub fn message_box (title: &str, body: &str) {
  // TODO: use run_process once it's commandline parsing is fixed
  std::process::Command::new ("window_manager_message_box")
    .args ([title, body, "--kind", "Error", "--font-size", "20"])
    .spawn ()
    .ok ();
}
