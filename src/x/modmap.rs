use super::Display;
use x11::{keysym, xlib::XFreeModifiermap};
use xcb::x::{KeyButMask, ModMask};

#[derive(Copy, Clone)]
pub struct ModifierMapping {
    numlock: ModMask,
    win: ModMask,
    alt: ModMask,
    // This uses a different type as we get a `KeyButMask` from events.
    allow_mask: KeyButMask,
}

impl ModifierMapping {
    /// Creates an uninitialized modifier mapping, use `refresh` before using it.
    pub fn new() -> Self {
        Self {
            numlock: ModMask::empty(),
            win: ModMask::empty(),
            alt: ModMask::empty(),
            allow_mask: KeyButMask::empty(),
        }
    }

    /// Refreshes the masks for the NumLock, Alt/Meta, and Super/Windows keys.
    /// This refreshes the instance and then also returns a copy of it.
    ///
    /// The Alt and Super keys use the topmost key found in the following lists.
    ///
    /// Alt/Meta:
    ///   - Alt_L
    ///   - Meta_L
    ///   - Alt_R
    ///   - Meta_R
    ///
    /// Super/Windows:
    ///   - Super_L
    ///   - Win_L
    ///   - Hyper_L
    ///   - Super_R
    ///   - Win_R
    ///   - Hyper_R
    pub fn refresh(&mut self, display: &Display) -> Self {
        log::trace!("Refreshing modifier mapping");
        let numlock_code = display.keysym_to_keycode(keysym::XK_Num_Lock);
        // Alt and Win codes are ordered by how much we like them, lower index
        // is better.
        let alt_codes = [
            display.keysym_to_keycode(keysym::XK_Alt_L),
            display.keysym_to_keycode(keysym::XK_Meta_L),
            display.keysym_to_keycode(keysym::XK_Alt_R),
            display.keysym_to_keycode(keysym::XK_Meta_R),
        ];
        let mut alt_match = alt_codes.len();
        let win_codes = [
            display.keysym_to_keycode(keysym::XK_Super_L),
            display.keysym_to_keycode(keysym::XK_Win_L),
            display.keysym_to_keycode(keysym::XK_Hyper_L),
            display.keysym_to_keycode(keysym::XK_Super_R),
            display.keysym_to_keycode(keysym::XK_Win_R),
            display.keysym_to_keycode(keysym::XK_Hyper_R),
        ];
        let mut win_match = win_codes.len();
        *self = Self::new();
        let modmap = display.get_modifier_mapping();
        let max_keypermod = unsafe { (*modmap).max_keypermod };
        // First 3 modifiers are `shift`, `lock`, and `control` which we don't
        // care about here, the last 5 are `mod1`~`mod5`.
        for modifier in 3..8 {
            for slot in 0..max_keypermod {
                let check = unsafe {
                    *(*modmap)
                        .modifiermap
                        .add((modifier * max_keypermod + slot) as usize)
                };
                let bit = ModMask::from_bits(1 << modifier).unwrap();
                if check == numlock_code {
                    self.numlock = bit;
                } else if let Some(match_index) = alt_codes.iter().position(|p| *p == check) {
                    if match_index < alt_match {
                        self.alt = bit;
                        alt_match = match_index;
                    }
                } else if let Some(match_index) = win_codes.iter().position(|p| *p == check) {
                    if match_index < win_match {
                        self.win = bit;
                        win_match = match_index;
                    }
                }
            }
        }
        unsafe { XFreeModifiermap(modmap) };
        let var = KeyButMask::from_bits_truncate((self.alt | self.win).bits());
        self.allow_mask = KeyButMask::SHIFT | KeyButMask::CONTROL | var;
        *self
    }

    /// Returns the mask for the NumLock key.
    pub fn numlock(&self) -> ModMask {
        self.numlock
    }

    /// Returns the mask for the Windows/Super key.
    pub fn win(&self) -> ModMask {
        self.win
    }

    /// Returns the mask for the Alt/Meta key.
    pub fn alt(&self) -> ModMask {
        self.alt
    }

    // Accessors for the constants for uniformity

    /// Returns the mask for the Shift key.
    pub fn shift(&self) -> ModMask {
        ModMask::SHIFT
    }

    /// Returns the mask for the CapsLock key.
    pub fn lock(&self) -> ModMask {
        ModMask::LOCK
    }

    /// Returns the mask for the Control key.
    pub fn control(&self) -> ModMask {
        ModMask::CONTROL
    }

    /// Clean the state of a key/button event. This removes the bits for toggled
    /// modifiers (CapsLock and NumLock) and translates the other mods to a
    /// common bit if neccessary (i.e. all bits that indicate that we consider
    /// to be the Alt key become `XK_Alt_L`).
    pub fn clean_mods(&self, state: KeyButMask) -> KeyButMask {
        state & self.allow_mask
    }
}

impl Default for ModifierMapping {
    fn default() -> Self {
        Self::new()
    }
}
