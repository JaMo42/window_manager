use crate::desktop_entry::{get_desktop_entry_data_dirs, DesktopEntry};
use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::fs;

static CACHE: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);

/// Caches the StartupWMClass field of all desktop files.
pub fn set_startup_wm_classes() {
    let mut lock = CACHE.lock();
    let cache = lock.get_or_insert_with(BTreeMap::new);
    for i in get_desktop_entry_data_dirs() {
        // paths are already checked in `get_desktop_entry_data_dirs`.
        // I guess we could get a reading error but they *should* be
        // readable.
        let dir = fs::read_dir(&i).unwrap();
        for dir_entry in dir.into_iter().filter(|x| x.is_ok()).map(Result::unwrap) {
            let path = format!("{}", dir_entry.path().display());
            if !path.ends_with(".desktop") {
                continue;
            }
            // TODO: could create a specialized function that only reads the
            // StartupWMCLass field, this would also mean that we don't
            // need to store it int he DesktopEntry structure.
            if let Some(entry) = DesktopEntry::new_from_path(&path) {
                if let Some(class) = entry.startup_wm_class {
                    let entry_name = dir_entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                    cache.insert(class, entry_name);
                }
            }
        }
    }
}

pub fn get_application_id(names: &[Option<&str>], fallback: Option<&str>) -> Option<String> {
    fn lookup(names: &[Option<&str>]) -> Option<(bool, String)> {
        // Try cached
        let lock = CACHE.lock();
        let cache = lock.as_ref().unwrap();
        for i in names.iter().filter(|x| x.is_some()) {
            if let Some(id) = cache.get(i.unwrap()) {
                return Some((false, id.clone()));
            }
        }
        // Try names
        for i in names.iter().filter(|x| x.is_some()) {
            let name = i.unwrap();
            if let Some(entry_name) = DesktopEntry::entry_name(name) {
                return Some((true, entry_name));
            }
        }
        // Try lowercase names
        for i in names.iter().filter(|x| x.is_some()) {
            let name = i.unwrap().to_lowercase();
            if let Some(entry_name) = DesktopEntry::entry_name(&name) {
                return Some((false, entry_name));
            }
        }
        None
    }
    if let Some((add_to_cache, id)) = lookup(names) {
        if add_to_cache {
            let mut lock = CACHE.lock();
            let cache = lock.get_or_insert_with(BTreeMap::new);
            for name in names.iter().flatten() {
                cache.insert(name.to_string(), id.clone());
            }
        }
        return Some(id);
    }
    fallback.map(ToOwned::to_owned)
}
