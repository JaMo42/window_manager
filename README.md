# window_manager

stacking window manager for X

![window_manager](./doc/screenshot.png)

[wallpaper](https://unsplash.com/photos/1wrjYqLqn8c)

## Dependencies

Written in rust, `cargo` required.

### Libraries

- `xcb`
- `xcb-icccm`
- `Xcursor`
- `gtk-3`

apt libraries: `libx11-xcb-dev libxcb-icccm4-dev libxcursor-dev libgtk-3-dev`

pacman libraries: `libxcb xcb-util-wm libxcursor gtk3`

### Optional

- [`grid-resize`](https://github.com/JaMo42/grid-resize) required if the `grid_resize` option is enabled (see [configuration](./doc/CONFIG.md)).
- `asound`
    - apt: `libasound2-dev`
    - pacman: `alsa-lib`
- `pulse`
    - apt: `libpulse-dev`
    - pacman: `libpulse`

Either ALSA (libasound) or PulseAudio (libpulse) are required for volume controls, PulseAudio is required for per-application volume mixing.
Their presence is detected when compiling with `make` and their implementations are enabled respectively.
If both are installed PulseAudio is preferred over ALSA since it provides per-application controls.

## Usage

Build and install a release build: `make release && sudo make install`

### Building

```sh
# Debug build for only the window manager (both commands do the same):
$ make
$ make debug
# Debug build including utility programs:
$ BUILD_ALL=1 make debug
# Release build including utility programs:
$ make release
```

Using `make` for building, the presence of the ALSA and PulseAudio libraries is
checked and their implementations are only enabled if available.

When building with `cargo build` both backends are used by default, use `--no-default-features` to disable them and the `alsa` or `pulse` features to enable only one of them.

Example: `cargo build --no-default-features --features alsa`

### Installing

```sh
# Install to /usr/local/bin
$ sudo make install
# Install to custom path
$ sudo INSTALL_PREFIX=/my/install/path make install
```

Installed programs (with default path):
- `/usr/local/bin/window_manager`
- `/usr/local/bin/window_manager_quit`
- `/usr/local/bin/window_manager_message_box`

If running the install command results in a rustup error it is trying to rebuild but cannot since it's running as super user.
Just run `make release` normally to fix this.

In addition to the window manager and its utility programs this will also install the `window_manager.desktop` file to `/usr/share/xsessions/window_manager.desktop`.

### Running

The window manager is visible to any display manager like `GDM`, `SDDM`, `LightDM`, or others via the `/usr/share/xsessions/window_manager.desktop` file installed by `make install`.

The window manager can be ran from a `.xinitrc` using:
```sh
exec window_manager
```

To run a debug build without installing it use the `run.sh` script:
- `./sun.sh` or `./run.sh startx`: starts the X server and the window manager, must be run from a tty
- `./run.sh xephyr` starts Xephyr with a single display and runs the window manager in it
- `./run.sh multi_mon` starts Xephyr with multiple monitors and runs the window manager in it

## Configuration

The window manager is configured using the `config.ini` file in `$XDG_CONFIG_HOME/window_manager` or `$HOME/.config/window_manager`.

See [configuration file format](./doc/CONFIG.md).

Applications that should run at startup are added to `autostartrc` in the same directory.

This is simply a bash script that is run on startup (so don't forget to add `&` to the end of command so they don't block execution).

An example configuration is included in the `config` directory.

### Pre-defined key bindings

- `Mod + Left Mouse Click` Focus the clicked window

- `Mod + Left Mouse Hold` Move the clicked window

- `Mod + Shift + Left Mouse Hold` Move the clicked window and snap it based on the resulting position
  - Left half:
    - Top/Bottom quarter: Snap to Top-/Bottom- Left
    - Otherwise: Snap to Left
  - Right half works the same way

- `Mod + Right Mouse Hold` Resize the clicked window

- `Alt + Tab` Windows-style window switching

## Status bar

The window manager has a builtin status bar. See the [configuration file format](./doc/CONFIG.md) for customization options.

The bar contains the following widgets:

- Left: Workspace display and switcher

- Right: Battery charge, volume, current time, and a system tray

Controls:

- Workspace switcher:
  - Click: Select workspace
  - Scroll: Scroll through workspaces

- Volume:
  - Left click: Toggle master volume mute
  - Right click: Open volume mixer
  - Scroll up: Increase master volume
  - Scroll down: Decrease master volume

## Acknowledgments

Some projects I used to learn how windows managers work.

- [qpwm](https://github.com/ssleert/qpwm/)
- [dwm](https://dwm.suckless.org/)
- [tinywm](https://github.com/mackstann/tinywm)
- [How X Window Managers Work, And How To Write One](https://jichu4n.com/posts/how-x-window-managers-work-and-how-to-write-one-part-i/)
- [Fvwm](https://www.fvwm.org/)

And [polybar](https://github.com/polybar/polybar) for the system tray.

### Icons

See [res/README.md](./res/README.md)
