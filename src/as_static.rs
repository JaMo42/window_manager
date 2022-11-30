pub trait AsStaticRef<T: ?Sized> {
  fn as_static_ref (&self) -> &'static T;
}

impl<T> AsStaticRef<T> for Box<T> {
  fn as_static_ref (&self) -> &'static T {
    unsafe {
      let sr: &'static _ = &*(self.as_ref () as *const T);
      sr
    }
  }
}

impl<T> AsStaticRef<T> for *const T {
  fn as_static_ref (&self) -> &'static T {
    unsafe { &**self }
  }
}

pub trait AsStaticMut<T: ?Sized> {
  fn as_static_mut (&mut self) -> &'static mut T;
}

impl<T> AsStaticMut<T> for Box<T> {
  fn as_static_mut (&mut self) -> &'static mut T {
    unsafe {
      let sr: &'static mut _ = &mut *(self.as_mut () as *mut T);
      sr
    }
  }
}

impl<T> AsStaticMut<T> for *mut T {
  fn as_static_mut (&mut self) -> &'static mut T {
    unsafe { &mut **self }
  }
}
