#[allow(clippy::module_inception)]
mod dock;
mod item;

pub use dock::Dock;

use crate::as_static::AsStaticMut;
use crate::client::Client;
use crate::core::*;
use crate::x::{Window, XNone};
use item::Item;
use std::ptr::NonNull;
use x11::xlib::*;

pub(self) static mut item_context: XContext = XNone as XContext;

pub(self) static mut instance: Option<Dock> = None;

fn the () -> &'static mut Dock {
  unsafe { instance.as_mut ().unwrap_unchecked () }
}

pub unsafe fn create () {
  instance = Some (Dock::create ((*config).dock_item_size));
  // This is called before we have any clients
  the ().keep_open (true);
}

pub unsafe fn destroy () {
  the ().destroy ();
}

pub unsafe fn click_item (event: &XButtonEvent) {
  let dock = the ();
  if let Some (ctx) = Window::from_handle (&display, event.window).find_context (item_context) {
    let item = (ctx as *mut Item).as_static_mut ();
    match event.button {
      Button1 => {
        item.click ();
      }
      Button2 => {
        item.new_instance ();
      }
      Button3 => {
        dock.keep_open (true);
        item.close_tooltip ();
        item.context_menu ();
      }
      _ => {}
    }
  }
}

pub unsafe fn cross (event: &XCrossingEvent) {
  let dock = the ();
  if event.type_ == LeaveNotify {
    dock.hide_after (10);
  } else {
    dock.window ().raise ();
  }
  display.sync (false);
}

pub unsafe fn cross_item (event: &XCrossingEvent) {
  let dock = the ();
  dock.cancel_hide ();
  dock.window ().raise ();
  if let Some (ctx) = Window::from_handle (&display, event.window).find_context (item_context) {
    let item: &'static mut Item = (ctx as *mut Item).as_static_mut ();
    item.redraw (dock.drawing_context (), event.type_ == EnterNotify);
    if event.type_ == EnterNotify {
      item.show_tooltip ();
    } else {
      item.close_tooltip ();
    }
  }
}

pub unsafe fn cross_show (event: &XCrossingEvent) {
  let dock = the ();
  if event.type_ == EnterNotify {
    dock.show ();
  } else if !dock.contains (event.x_root, event.y_root) {
    dock.hide_after (500);
  }
}

pub fn keep_open (yay_or_nay: bool) {
  the ().keep_open (yay_or_nay);
}

pub unsafe fn add_client (client: &mut Client) {
  the ().add_client (NonNull::new_unchecked (client as *mut Client));
}

pub unsafe fn remove_client (client: &Client) {
  the ().remove_client (client);
}

pub unsafe fn focus (client: &Client) {
  the ().update_focus (client);
}

pub unsafe fn update_urgency (client: &mut Client) {
  the ().update_urgency (client);
}

pub unsafe fn resize () {
  the ().resize ();
}
