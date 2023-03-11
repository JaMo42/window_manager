use std::sync::Mutex;

macro_rules! cached_paths {
    {
        $(
            $(#[doc = $doc:expr])?
            $vis:vis fn $name:ident() -> String $body:block
        )*
    } => {
        $(
            $(#[doc = $doc])?
            $vis fn $name() -> String {
                static CACHED: Mutex<Option<String>> = Mutex::new(None);
                if let Some(cached) = &*CACHED.lock().unwrap() {
                    return cached.clone();
                }
                let value = $body;
                *CACHED.lock().unwrap() = Some(value.clone());
                value
            }
        )*
    }
}

cached_paths! {
    /// Returns the path to the current users config directory.
    fn base_config_dir() -> String {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            config_home
        } else {
            format!("{}/.config", std::env::var("HOME").unwrap())
        }
    }

    /// Returns the path to the window managers config directory.
    pub fn config_dir() -> String {
        format!("{}/{}", base_config_dir(), "window_manager")
    }
}

/// Returns the path to the color scheme directory.
pub fn colors_dir() -> String {
    format!("{}/{}", config_dir(), "colors")
}

/// Returns the path to the log file.
pub fn log_path() -> String {
    format!("{}/log.txt", config_dir())
}

/// Returns the path to the config file.
pub fn config_path() -> String {
    format!("{}/config.ini", config_dir())
}

/// Returns the path to the autostart script.
pub fn autostart_path() -> String {
    format!("{}/autostartrc", config_dir())
}
