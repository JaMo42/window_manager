use freedesktop_entry_parser::parse_entry;
use std::{cell::RefCell, collections::HashMap, env::var, fs::metadata, rc::Rc};

use crate::desktop_entry::DesktopEntry;

// https://specifications.freedesktop.org/icon-theme-spec/latest/

fn find_icon_dir(name: &str) -> Option<String> {
    // Spec:
    // By default, apps should look in $HOME/.icons (for backwards
    // compatibility), in $XDG_DATA_DIRS/icons and in /usr/share/pixmaps
    // (in that order)
    //
    // Pretty sure /usr/share/pixmaps is only meant for actualy icon lookup,
    // not a theme location; but that's just based on what I have in it on my
    // current machine.
    //
    // ~/.local/share/icons is not in the spec but I've seen it used as well.
    if let Ok(home) = var("HOME") {
        let path = format!("{}/.icons/{}", home, name);
        if metadata(&path).is_ok() {
            return Some(path);
        }
        let path = format!("{}/.local/share/icons/{}", home, name);
        if metadata(&path).is_ok() {
            return Some(path);
        }
    }
    let xdg_data_dirs = match var("XDG_DATA_DIRS") {
        Ok(dirs) => dirs,
        // XXX: for desktop entries local comes first, but I have no idea why
        //      I did it like that, this seems better to me now.
        Err(_) => "/usr/share/:/usr/local/share/".to_string(),
    };
    for dir in xdg_data_dirs.split(':') {
        let path = format!("{}/icons/{}", dir, name);
        if metadata(&path).is_ok() {
            return Some(path);
        }
    }
    None
}

#[derive(Default, Debug)]
pub struct IconRegistry {
    themes: Vec<IconTheme>,
}

impl IconRegistry {
    // This the main entry point of the icon system, which is called with the
    // configured icon theme name.
    pub fn new(theme: &str) -> std::io::Result<Self> {
        log::trace!("Looking for icon theme: {}", theme);
        // Themes are inserted such that the insertion order is the correct
        // order we want for lookups.
        let mut themes = HashMap::new();
        let mut in_order = Vec::new();
        match find_icon_dir(theme).or_else(|| {
            log::warn!("Configured theme not found: {}", theme);
            find_icon_dir("hicolor")
        }) {
            Some(path) => {
                log::trace!("  Found main theme at: {}", path);
                let name = theme;
                let theme = Rc::new(RefCell::new(IconTheme::default()));
                themes.insert(name.to_string(), theme.clone());
                in_order.push(name.to_string());
                theme
                    .borrow_mut()
                    .create(path, &mut themes, &mut in_order)?;
            }
            // This is exceptional; `hicolor` must exist
            None => panic!("No icon theme found at all"),
        };
        // Use Adwaita as additional fallback.
        if theme != "Adwaita" {
            if let Some(path) = find_icon_dir("Adwaita") {
                log::trace!("  Found additional fallback theme at: {}", path);
                let name = "Adwaita";
                let theme = Rc::new(RefCell::new(IconTheme::default()));
                themes.insert(name.to_string(), theme.clone());
                // And we want to use it over hicolor as a fallback.
                if let Some(before) = in_order.iter().position(|n| n == "hicolor") {
                    in_order.insert(before, name.to_string());
                } else {
                    in_order.push(name.to_string());
                }
                theme
                    .borrow_mut()
                    .create(path, &mut themes, &mut in_order)?;
            }
        }
        Ok(Self {
            themes: in_order
                .into_iter()
                .map(|name| {
                    RefCell::into_inner(Rc::into_inner(themes.remove(&name).unwrap()).unwrap())
                })
                .filter(|theme| !theme.directories.is_empty())
                .collect(),
        })
    }

    pub fn lookup(&self, name: &str) -> Option<String> {
        if name.starts_with('/') {
            return Some(name.to_string());
        }
        for theme in &self.themes {
            for directory in &theme.directories {
                let path = format!("{}/{}.svg", directory, name);
                if metadata(&path).is_ok() {
                    return Some(path);
                }
            }
        }
        let path = format!("/usr/share/pixmaps/{}.svg", name);
        if metadata(&path).is_ok() {
            return Some(path);
        }
        None
    }

    pub fn lookup_app_app_icon(&self, name: &str) -> Option<String> {
        if name.starts_with('/') {
            return Some(name.to_string());
        }
        for theme in &self.themes {
            if let Some(app_dir) = theme.app_dir {
                let path = format!("{}/{}.svg", theme.directories[app_dir], name);
                if metadata(&path).is_ok() {
                    return Some(path);
                }
            }
        }
        self.lookup(name)
    }

    pub fn lookup_app_icon(&self, app_name: &str) -> Option<String> {
        if app_name.is_empty() {
            return None;
        }
        match DesktopEntry::new(app_name) {
            Some(entry) if entry.icon.is_some() => {
                self.lookup_app_app_icon(unsafe { entry.icon.as_deref().unwrap_unchecked() })
            }
            _ => self.lookup_app_app_icon(app_name),
        }
    }
}

#[derive(Default, Debug)]
pub struct IconTheme {
    directories: Vec<String>,
    /// Index of `apps` folder in `directories`
    app_dir: Option<usize>,
}

impl IconTheme {
    /// Constructor, but theme is inserted into the registry first to easily
    /// manage correct inheritance precedence, hence it works on a reference.
    ///
    /// `basepathname` is the absolute directory of the `index.theme` file.
    fn create(
        &mut self,
        basepathname: impl ToString,
        known: &mut HashMap<String, Rc<RefCell<IconTheme>>>,
        insertion_order: &mut Vec<String>,
    ) -> std::io::Result<()> {
        let basepathname = basepathname.to_string();
        let entry = parse_entry(format!("{}/index.theme", basepathname))?;
        let icon_theme = entry.section("Icon Theme");
        let inherits = icon_theme.attr("Inherits").unwrap_or("hicolor");
        for name in inherits.split(',') {
            // Note: entry API does not work with transient lookup so we
            // would need `to_string` the name here to use it.
            if known.contains_key(name) {
                continue;
            }
            if let Some(path) = find_icon_dir(name) {
                log::trace!("  Inherited theme at: {}", path);
                let theme = Rc::new(RefCell::new(IconTheme::default()));
                known.insert(name.to_string(), theme.clone());
                insertion_order.push(name.to_string());
                theme.borrow_mut().create(path, known, insertion_order)?;
            } else {
                log::warn!("Inherited icon theme not found: {}", name);
            }
        }
        if let Some(directories) = icon_theme.attr("Directories") {
            // The themes I've looked at were all ordered by size so reversing
            // here gives biggest first which I will consider as the best since
            // we lookup the icons independently of the size they will be drawn
            // at.
            // We will get unused duplicates here since we don't stop once we
            // have all the categories but I will ignore that for now.
            for subdirname in directories.split(',').rev() {
                let subdir = entry.section(subdirname);
                if subdir.attr("Type") == Some("Scalable") && subdir.attr("Scale").is_none() {
                    self.directories
                        .push(format!("{}/{}", basepathname, subdirname));
                    if !self.app_dir.is_none() && subdirname.contains("apps") {
                        self.app_dir = Some(self.directories.len() - 1);
                    }
                }
            }
        } else {
            log::error!("No `Directories` in index.theme: {}", basepathname);
        }
        Ok(())
    }
}
