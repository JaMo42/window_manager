# window_manager

stacking window manager for X

![window_manager](./doc/screenshot.png)

[wallpaper](https://unsplash.com/photos/1wrjYqLqn8c)

## Dependencies

Written in rust, `cargo` required.

### Libraries

- `Xcursor`
- `asound` (ALSA)
- `pulse` (PulseAudio)

### Optional

- [`grid-resize`](https://github.com/JaMo42/grid-resize) required if the `grid_resize` option is enabled (see [configuration](./doc/CONFIG.md)).

## Compiling

### Debug

```sh
$ cargo build
$ ./run.sh
```

This launches the X server and runs the window manager locally so it has to be ran from a tty.

Methods of running:

- `./run.sh` or `./run.sh startx`: starts the X server and the window manager, must be called from a tty.

- `./run.sh xephyr`: starts Xephyr with a single displayed and runs the window manager in it.

- `./run.sh multi_mon`: starts Xephyr with multiple monitors and runs the window manager in it.

### Release

```
$ cargo build --realse
$ ./install.sh
```

This installs the windows manager onto the system, it can now be ran from a `.xinitrc` using:

```sh
exec window_manager
```

Or for a display manager like `GDM`, `SDDM`, `LightDM`, add this to `/usr/share/xsessions/window_manager.desktop`:

```desktop
[Desktop Entry]
Name=window_manager
Comment=Session running window_manager
Exec=window_manager
Type=Application
```

(the `install.sh` script will ask you if want to create this file).

### Installation script

Running `install.sh` will:

- compile the program if `target/realease/window_manager` does not exist

- copy the program to `$INSTALL_PREFIX/window_manager` (default for `INSTALL_PREFIX` is `/usr/local/bin`)

- For each of these files, asks if you want to copy them to the corresponding location:
  - `config` -> `$CONFIG_PREFIX/window_manager/config`
  - `autostartrc` -> `$CONFIG_PREFIX/window_manager/autostratrc` (You probably want your own file instead of the example)
  - `window_manager.desktop` -> `/usr/share/xsessions/window_manager.desktop`

  (default for `CONFIG_PREFIX` is `$XDG_CONFIG_HOME` or `$HOME/.config`)

## Configuration

The window manager is configured using the `config.ini` file in `$XDG_CONFIG_HOME/window_manager` or `$HOME/.config/window_manager`.

See [configuration file format](./doc/CONFIG.md).

Applications that should run at startup are added to `autostartrc` in the same directory.

This is simply a bash script that is run on startup (so don't forget to add `&` to the end of command so they don't block execution).

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
