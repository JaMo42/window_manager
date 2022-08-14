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
color <Element> #RGB
color <Element> #RRGGBB
color <Element> #RRRRGGGGBBBB
```
Set the [`<Element>`](#color-elements) to the hex-color.

```
color <Element1> <Element2>
```
Set the [`<Element1>`](#color-elements) to the same color as `<Element2>`.
Can be used before `Element2` is defined.

```
def_color <Name> #RGB
def_color <Name> #RRGGBB
def_color <Name> #RRRRGGGGBBBB
```
Define a named color that can be used to set other colors. These cannot contain links.

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

```
window_title_position left|center|right
```
Set the window title alignment.

```
left_buttons [buttons...]
```
Set the left [window buttons](#window-buttons) (not that by default the window title is on the left).

```
right_buttons [buttons...]
```
Set the right [window buttons](#window-buttons).

```
button_icon_size <percentage>
```
Set the relative size of window button icons; the buttons themselves are always the same height as the title bar.

```
circle_buttons
```
Enable circle buttons.

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

## Height values

A height value can be either `<number>` or `+<number>`, the former specifying a absolute value and the letter a value that's added on the font-size of the element.
If the number for the absolute value is 0, the height of the font is used.

## Window buttons

- `close` Closes the window

- `maximize` Toggles between maximized and un-snapped

- `minimize` Minimizes the window

## Color elements

| **Element** | **Description**
| --- | ---
| Focused | Border of the focused window
| FocusedText | Text on the border of the focused window
| Normal | As `Focused` but for normal windows
| NormalText |
| Selected | As `Focused` but for the selected window during window switching
| SelectedText |
| Urgent | As `Focused` but for windows that demand attention
| UrgentText |
| CloseButton | Default color of the close button
| CloseButtonHovered | Color of the close button when the mouse is above it
| MaximizeButton | As `CloseButton` but for the maximize button
| MaximizeButtonHovered |
| MinimizeButton | As `CloseButton` but for the minimize button
| MinimizeButtonHovered |
| Background | Color of the root window
| **Colors for the builtin bar** |
| Bar::Background | Background color
| Bar::Text | Color of battery and clock widget
| Bar::Workspace | Color for normal workspace indicators
| Bar::WorkspaceText | Text color for normal workspace indicators
| Bar::ActiveWorkspace | As `Bar::Workspace` but for the active workspace
| Bar::ActiveWorkspaceText |
| Bar::UrgentWorkspace | as `Bar::Workspace` but for workspaces that contain windows  that demand attention
| Bar::UrgentWorkspaceText |

### Circle buttons

If circle buttons are enabled, `<Button>` becomes the circle color for normal windows and `<Button>Hovered` becomes the circle color for focused windows.
The color of the actual icon gets derived from the `<Button>Hovered` color.
