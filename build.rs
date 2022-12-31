fn main() {
  println!("cargo:rustc-link-lib=X11");
  println!("cargo:rustc-link-lib=Xft");
  println!("cargo:rustc-link-lib=XRes");
  println!("cargo:rustc-link-lib=Xinerama");
  println!("cargo:rustc-link-lib=Xfixes");
}
