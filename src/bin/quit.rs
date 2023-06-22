use gdk::{Screen, WindowTypeHint};
use gio::{BusType, Cancellable, DBusCallFlags};
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, CssProvider, IconSize, Image, Orientation,
    PositionType, StyleContext,
};

struct Action {
    label: &'static str,
    icon_name: &'static str,
    return_value: &'static str,
}

static ACTIONS: [Action; 4] = [
    Action {
        label: "Logout",
        icon_name: "system-log-out",
        return_value: "logout",
    },
    Action {
        label: "Sleep",
        icon_name: "system-suspend",
        return_value: "sleep",
    },
    Action {
        label: "Restart",
        icon_name: "system-restart",
        return_value: "restart",
    },
    Action {
        label: "Shutdown",
        icon_name: "system-shutdown",
        return_value: "shutdown",
    },
];

static mut THE_APP: *const Application = std::ptr::null();

fn set_font_size(size: usize) {
    let screen = Screen::default().unwrap();
    let gtk_provider = CssProvider::new();
    StyleContext::add_provider_for_screen(
        &screen,
        &gtk_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let css = format!("* {{ font-size: {}px; }}", size);
    if let Err(error) = gtk_provider.load_from_data(css.as_bytes()) {
        eprintln!("Failed to load css: {}", error);
    }
}

fn quit() {
    unsafe {
        if !THE_APP.is_null() {
            (*THE_APP).quit();
        }
    }
}

fn send_choice(choice: &'static str) {
    const NAME: &str = "com.github.JaMo42.window_manager.SessionManager";
    const PATH: &str = "/com/github/JaMo42/window_manager/SessionManager";
    match gio::bus_get_sync(BusType::Session, Cancellable::NONE) {
        Ok(connection) => {
            let params = (choice,).to_variant();
            if let Err(error) = connection.call_sync(
                Some(NAME),
                PATH,
                NAME,
                "Quit",
                Some(&params),
                None,
                DBusCallFlags::NONE,
                -1,
                Cancellable::NONE,
            ) {
                eprintln!(
                    "Failed to call org.window_manager.SessionManager.Quit: {}",
                    error
                );
            } else {
                eprintln!("Success");
            }
        }
        Err(error) => {
            panic!("Failed to connect to debus: {}", error);
        }
    }
}

fn build() -> Application {
    let app = Application::builder()
        .application_id("com.github.JaMo42.window_manager.quit")
        .build();

    let margin = 10;
    let padding = 5;
    let button_size = 240;
    let width = 2 * margin + ACTIONS.len() as i32 * (padding + button_size) - padding;
    let height = 2 * margin + button_size;

    app.connect_activate(move |app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Quit")
            .default_width(width)
            .default_height(height)
            .type_hint(WindowTypeHint::Dialog)
            .build();

        let container = Box::new(Orientation::Horizontal, 0);
        container.set_margin(10);
        window.add(&container);

        for action in ACTIONS.iter() {
            let icon = Image::from_icon_name(Some(action.icon_name), IconSize::Dialog);
            let button = Button::builder()
                .label(action.label)
                .image(&icon)
                .image_position(PositionType::Top)
                .build();
            button.set_size_request(button_size, button_size);
            button.connect_clicked(|_| {
                send_choice(action.return_value);
                quit();
            });
            container.pack_start(&button, true, true, 5);
        }

        window.show_all();
    });

    app
}

fn main() {
    if let Err(error) = gtk::init() {
        panic!("Failed to initialize gtk: {}", error);
    }
    set_font_size(24);
    let app = build();
    unsafe {
        THE_APP = &app;
    }
    app.run();
}
