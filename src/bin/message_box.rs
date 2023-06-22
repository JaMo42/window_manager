use clap::Parser;
use gdk::Screen;
use gtk::prelude::*;
use gtk::{ButtonsType, CssProvider, MessageDialog, MessageType, StyleContext};

#[derive(Parser)]
struct CommandLine {
    title: String,
    body: String,

    /// Info, Warning, or Error
    #[arg(long, default_value_t = String::from ("Info"))]
    kind: String,

    #[arg(long, default_value_t = 18)]
    font_size: usize,
}

fn show(kind: MessageType, title: &str, body: &str, font_size: usize) {
    if let Err(error) = gtk::init() {
        panic!("Failed to initialize gtk: {}", error);
    }
    let screen = Screen::default().unwrap();
    let gtk_provider = CssProvider::new();
    StyleContext::add_provider_for_screen(
        &screen,
        &gtk_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let css = format!("* {{ font-size: {}px; }}", font_size);
    if let Err(error) = gtk_provider.load_from_data(css.as_bytes()) {
        eprintln!("Failed to load css: {}", error);
    }

    let dialog = MessageDialog::builder()
        .message_type(kind)
        .buttons(ButtonsType::Ok)
        .text(title)
        .secondary_text(body)
        .build();

    if kind == MessageType::Error {
        dialog.set_urgency_hint(true);
    }

    dialog.run();
}

fn main() {
    let cmdline = CommandLine::parse();
    let kind = match cmdline.kind.as_str() {
        "Info" => MessageType::Info,
        "Warning" => MessageType::Warning,
        "Error" => MessageType::Error,
        _ => panic!("Invalid message box kind, should be one of: Info, Warning, Error"),
    };

    show(kind, &cmdline.title, &cmdline.body, cmdline.font_size);
}
