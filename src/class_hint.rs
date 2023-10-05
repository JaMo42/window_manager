use crate::x::{GetProperty, PropertyValue, SetProperty, Window};

#[derive(Debug, Default)]
pub struct ClassHint {
    pub class: String,
    pub name: String,
}

impl ClassHint {
    pub fn get(window: &Window) -> Option<Self> {
        let display = window.display();
        let data = window.get_string_property(display, display.atoms.wm_class)?;
        let mut it = data.split('\0');
        let name = it.next()?.to_owned();
        let class = it.next()?.to_owned();
        Some(Self { class, name })
    }

    pub fn new(class: &str, name: &str) -> Self {
        Self {
            class: class.to_string(),
            name: name.to_string(),
        }
    }

    /// Set the class hint on the given window.
    pub fn set(&self, window: &Window) {
        let data = format!("{}\0{}\0", self.name, self.class);
        let display = window.display();
        window.set_property(display, display.atoms.wm_class, PropertyValue::String(data));
    }
}
