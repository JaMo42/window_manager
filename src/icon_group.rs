use crate::as_static::AsStaticMut;
use crate::draw::SvgResource;
use crate::paths;

/// Set of icons, only 1 can be loaded at any time.
pub struct IconGroup<const N: usize> {
  names: [&'static str; N],
  loaded: Option<(usize, Box<SvgResource>)>,
}

impl<const N: usize> IconGroup<N> {
  pub const fn new(names: [&'static str; N]) -> Self {
    Self {
      names,
      loaded: None,
    }
  }

  pub const fn len(&self) -> usize {
    N
  }

  pub fn get(&mut self, idx: usize) -> &'static mut SvgResource {
    let name = &self.names[idx];
    if let Some((loaded_idx, icon)) = &mut self.loaded {
      if idx == *loaded_idx {
        return icon.as_static_mut();
      }
    }
    let pathname = format!("{}/{}", unsafe { &paths::resource_dir }, name);
    self.loaded = Some((idx, SvgResource::open(&pathname).unwrap()));
    return self.loaded.as_mut().unwrap().1.as_static_mut();
  }
}
