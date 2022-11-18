# Configuration

The main configuration is a TOML file in the configuration directory called `config.toml`.

### `general` Section

Key | Description | Default
---|---|---
`meta_window_classes` | Classes to use as meta windows; these windows are visible from each workspace, are undecorated, and cannot be interacted with. Additionally, windows with the title `window_manager_bar` are also meta windows. | `[]`
`default_notification_timeout` | Default time desktop notifications are displayed for if no time is specified, if set to `0` notifications never expire. | `6000`

### `layout` section

Key | Description | Default
---|---|---
`workspaces` | Number of workspaces. | `1`
`gaps` | Width of inner gaps for snapped windows. | `0`
`pad` | Padding from screen edges for snapped windows, values are [Top, Bottom, Left, Right]. | `[0, 0, 0, 0]`

### `window` section

Key | Description | Default
---|---|---
`border` | Width of window borders. This value sets the width on the left, right, and bottom. | `0`
`title_font` | Font used for window titles. | `sans 14`
`title_bar_height` | Height for the title bar of each window (top border) | `+2`
`title_alignment` | Window title alignment, `Left`, `Center`, or `Right`. | `Left`
`right_buttons` | Left [window buttons](#window-buttons), | `[]`
`left_buttons` | Right [window buttons](#window-buttons). | `[]`
`icon_size` | Percent size of window icons relative to the title bar, if the percentage is `0` window icons are disabled. | `0`
`circle_buttons` | Enable circle buttons. | `false`
`button_icon_size` | Percent size of button icons relative to the title bar height. | `75`

### `theme` section

Key | Description | Default
---|---|---
`colors` | [Color scheme](#color-schemes) name | `default`
`icons` | Icon theme name, must be the name of a directory in `/usr/share/icons` which must have `48x48` subfolder. Both the theme folder and the `48x48` folder mat be symbolic links. | `Papirus`

### `keys`

Key | Description | Default
---|---|---
`mod` | User-defined modifier. This is used for mouse key bindings as well as the `Mod` modifier in key bindings. | `Win`

### `keys.bindings`

This is a table where each key is a `+`-separated list of modifiers and a key which is set to the action given in the value.
The modifiers and key list may contain spaces.
If the actions starts with a `$`, the rest of the value is used a command to run when the key is pressed.
The commandline for command-actions may contain strings which are delimited by double or single quotes, a `\` escapes the character after it (only has an effect on other blackslashes, spaces, and string delimiters).

Examples:

```toml
[keys.bindings]
'Win+Q' = "close_window"
'Win+Shift+Q' = "quit"
'Win+Return' = "$ xterm"

# Escaping examples:
key = "$ command one\\ argument\\ with\\ spaces"
#                ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Results in: `one argument with spaces`
key = "$ command 'one argument as a string'"
#                 ~~~~~~~~~~~~~~~~~~~~~~~~
# Results in: `one argument as a string`
key = "$ command \"one argument containing \\\"another string\\\"\""
#                  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
# Results in: `one argument containing "another string"`
```

The underlined sections are each a single argument to the command, notice the double-escaping as TOML strings already have their own escaping.

The key names must adhere to the requirements of `XStringToKeysym`, that is:

```
Standard KeySym names are obtained from <X11/keysymdef.h> by removing the XK_ prefix from each name.
KeySyms that are not part of the Xlib standard also may be obtained with this function.
The set of KeySyms that are available in this manner and the mechanisms by which Xlib obtains them is implementation-dependent.
```

This does not include the modifiers, which must be one of `Shift`, `Ctrl`, `Win`, `Alt`, and `Mod` (the user-defined modifier).

### `bar`

Key | Description | Default
---|---|---
`font` | Font used for the status bar | `sans 14`
`opacity` | Percentage opacity of the bar. Note that the window manager does not have its own compositor so this only works if one like `picom` is running. | `100`
`height` | Height of the bar | `+5`
`time_format` | Format for the date-time widget, uses strftime format. | `%a %b %e %H:%M %Y`
`power_supply` | Power supply used by the battery widget. This should be the name of a folder in `/sys/class/power_supply`. If the given value does not exist the widget is disabled. | `BAT0`
`update_interval` | Interval in milliseconds in which the bar is automatically updated. Use `0` to disable automatic updates (it will still get updated by various events). | `10000`

## Modifiers

- `Win` Windows key

- `Alt` Alt

- `Shift` Shift

- `Ctrl` Control

- `Mod` The user-defined modifier

## Actions

- `close_window` Closes the focused window.

- `quit` Quits the window manager.

- `quit_dialog` Opens the quit dialog, this lets you choose between logging out, sleeping, rebooting, and shutting down.

- `snap_maximized` Snaps the focused window into the maximized position (not fullscreen).

- `snap_left` / `snap_right` Snap the focused window to the left/right half of the screen.
  If the window was snapped to the top/bottom on the opposite side, it stays at the top/bottom.
  If it was snapped to the top/bottom on the same side, it gets snapped to the full height.

- `snap_up` / `snap_down` If the focused window is snapped to the left/right, snap it to the top/bottom quarter.

- `unsnap_or_center` If the focused window is snapped, un-snap it, restoring it to it's position before it was last snapped. If it is not snapped, center it.

- `minimize` Minimizes the focused window, it can be made visible again using the alt+tab window switching or the `raise_all` action.

- `raise_all` Un-minimizes all windows on the current workspace.

- `unsnap_or_minimize` If the focused window is snapped, un-snap it, otherwise minimize it.

### Volume control

These are wrappers around the `amixer` command, they all affect the `Master` device.

- `increase_volume` increases the volume by 5%

- `decrease_volume` decreases the volume by 5%

- `mute_volume` toggles volume muting and sends a desktop notification whether volume is on or off

## Height values

A height value can be either `"<number>"` or `"+<number>"`, the former specifying a absolute value and the letter a value that's added on the font-size of the element.
If the number for the absolute value is 0, the height of the font is used.

## Window buttons

- `close` Closes the window

- `maximize` Toggles between maximized and un-snapped

- `minimize` Minimizes the window

## Color schemes

Color schemes are located at `colors/name` relative to the configuration directory.

They consist of a simple format where every line either sets of defines a color:

```
# This is a comment

# This defines a named color which can be used when setting a color:
def_color Name #123456

# This sets the color of an element:
color Element #123456
# or with a named color:
color Element Name
```

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
| NotificationBackground | Background color for desktop notifications
| NotificationText | Text color for desktop notifications

### Circle buttons

If circle buttons are enabled, `<Button>` becomes the circle color for normal windows and `<Button>Hovered` becomes the circle color for focused windows.
The color of the actual icon gets derived from the `<Button>Hovered` color.
