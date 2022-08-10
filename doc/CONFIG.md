# Configuration

The configuration file uses a simple format where each non-blank line contains either a comment or a definition.

Comment lines start with a `#` and are ignored.

## Definitions

```
workspaces <N>
```
Sets the number of workspaces.

```
gaps <N>
```
Sets the width of inner gaps for snapped windows.

```
pad <Top> <Bottom> <Left> <Right>
```
Sets the padding from the respective side of the desktop for snapped windows (useful for status bars).

```
border <N>
```
Sets the width of window borders. This value sets the width on the left, right, and bottom.

```
meta <Class>
```
Specifies windows with class `<Class>` to be meta-windows, these windows are visible from each workspace, are undecorated, and cannot be interacted with.
Additionally, windows with the title `window_manager_bar` are also meta windows.

```
mod <Mod>
```
Specifies `<Mod>` to be the user-defined modifier. This is used for mouse key bindings as well as the `Mod` modifier in `bind` definitions (has to be defined before).
If this is absent from the config file, the Windows key is used.

Warning: You *can* set this to `Shift` but it will break window moving as the pre-defined key for snap-moving just adds `Shift` to the user-modifier.

```
bind [<Mod>+...]<Key> <Action>
bind [<Mod>+...]<Key> $ <Command>
```
Bind the given key with the given [modifiers](#modifiers) (i.e. `Win+Up` or `Win+Shift+Q`) to either the given [`<Action>`](#actions), or command.
In the `<Command>` case, everything after the `$` is the process to launch.

The key names must adhere to the requirements of `XStringToKeysym`, that is:
```
Standard KeySym names are obtained from <X11/keysymdef.h> by removing the XK_ prefix from each name.
KeySyms that are not part of the Xlib standard also may be obtained with this function.
The set of KeySyms that are available in this manner and the mechanisms by which Xlib obtains them is implementation-dependent.
```

```
color <Element> #RRGGBB
```
Set the [`<Element>`](#color-elements) to the hex-color.

```
color <Element1> <Element2>
```
Set the [`<Element1>`](#color-elements) to the same color as `<Element2>`.
Can be used before `Element2` is defined.

```
bar_font <font>
```
Set the font of the status bar.
`<font>` is a pango font description, example: `Noto Sans 16` or `sans bold 18`.

```
bar_opacity <percentage>
```
Set the status bar opacity, `percentage` is a number between 0 and 100.

```
bar_time_format <format>
```
Set the format string for the time widget (man strftime).

```
bar_height <height>
```
Set the height of the status bar; see [Height values](#height-values).

```
window_title_font <font>
```
Set the font used for window titles.

```
window_title_height <height>
```
Set the height for the title bar of each window (top border); see [Height values](#height-values).

## Modifiers

- `Win` Windows key

- `Alt` Alt

- `Shift` Shift

- `Ctrl` Ctrl

- `Mod` The user-modifier

## Actions

- `close_window` Closes the focused window.

- `quit` Quits the window manager.

- `snap_maximized` Snaps the focused window into the maximized position (not fullscreen).

- `snap_left` / `snap_right` Snap the focused window to the left/right half of the screen.
  If the window was snapped to the top/bottom on the opposite side, it stays at the top/bottom.
  If it was snapped to the top/bottom on the same side, it gets snapped to the full height.

- `snap_up` / `snap_down` If the focused window is snapped to the left/right, snap it to the top/bottom quarter.

- `unsnap_or_center` If the focused window is snapped, un-snap it, restoring it to it's position before it was last snapped. If it is not snapped, center it.

## Color elements

- `Focused` the focused window

- `Normal` un-focused windows

- `Urgent` windows that demand attention

- `Select` the pending windows during window switching

- `Background` the desktop background (use an external program like `feh` to set a background image)

- Bar colors

  - `Bar::Background` background color of the bar

  - `Bar::Text` default text color

  - `Bar::Workspace` background color for non-active workspace indicators

  - `Bar::WorkspaceText` respective text color

  - `Bar::ActiveWorkspace` background color for non-active workspace indicators

  - `Bar::ActiveWorkspaceText` respective text color

  - `Bar::UrgentWorkspace` background color for indicators of workspaces which contain windows demanding attention

  - `Bar::UrgentWorkspaceText` respective text color

## Height values

A height value can be either `<number>` or `+<number>`, the former specifying a abosulte value and the letter a value that's added on the font-size of the element.
If the number for the absolute value is 0, the height of the font is used.
