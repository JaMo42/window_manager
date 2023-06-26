# Configuration

The configuration use a format somewhere betweem TOML and ini, they use the
`.ini` file extension for syntax highlighting.

## Format

The basic format is just like ini:

```ini
[section]
key = value

# Sections can be nested
# either like this:
[nested.section]
key = value

# or like this:
[nested]
# ...
[.section]  # leading '.' appends the name to the previous section
key = value
```

The difference is that we have set value types for the various things we need.

## Types

identifies | description | examples
---|---|---
`bool` | `true` or `false` |
`uint` | a unsigned decimal integer | `21`
`float` | a real number | `3.141`
`size` | like `float` but may be be suffixed by one of `px`, `mm`, `cm`, `em`, `%`. | See following rows
&nbsp; | `px` or no prefix: number of pixels | `10`
&nbsp; | `mm` or `cm`: physical size in millimeters/centimeters, based on monitor DPI | `2mm`, `1cm`
&nbsp; | `em`: multiple of font size, only available for certain values* | `1.1em`
&nbsp; | `%`: relative to some superior element, only available for certain values* | `80%`
`string` | a delimited string | `'single quoted'`, `"double quoted"`
`font` | alias for `string` | `'sans 14'`
`alignment` | `left`, `center`, or `right` |
`[T]` | a list where `T` is one of the other types. This is the only value that can span multiple lines. | `['minimize', 'maximize', 'close']`, `(16, 9)`
`[T; N]` | a list of `T` that must have `N` elements | `[1mm, 1mm, 1mm, 1mm]`
`color` | a color value or link | See following rows
&nbsp; | `#RRGGBB` or `#RRGGBBAA`: a hex color value | `#428BCA`, `#161514CC`
&nbsp; | `rgb(r, g, b)` or `rgba(r, g, b, a)`: components given as `uint`s | `rgb(235, 64, 52)`, `rgba(22, 21, 20, 204)`
&nbsp; | `foo.bar`: link to another element | `window.buttons.close`

*: Size values will indicate when they can be relative to font/superior element sizes.

## Sections

### [general]

Key and type | Description | Default
---|---|---
`meta_window_classes` <br> `[string]` | classes to use as meta windows; these windows are visible from each workspace, are undecorated, and cannot be interacted with. Additionally, windows with the title window_manager_bar are also meta windows. | `[]`
`default_notification_timeout` <br> `uint` | default time (milliseconds)desktop notifications are displayed for if no time is specified, if set to 0 notifications never expire. | `6000`
`double_click_time` <br> `uint` | maximum time after which a second click is considered a double-click in milliseconds. | `500`
`grid_resize` <br> `bool` | run [grid-resize](https://github.com/JaMo42/grid-resize) when holding the left and right mouse buttons while moving a window. | `false`
`grid_resize_grid_size` <br> `[uint; 2]` | the vertical and horizontal columns for grid-resize. | `(16, 9)`
`grid_resize_love` <br> `bool` | run grid-size in live mode. | `false`
`scale_base_fonts` <br> `bool` | see [fonts](#fonts). | `true`

### [layout]

Key and type | Description | Default
---|---|---
`workspaces` <br> `uint` | number of workspaces. | `1`
`gaps` <br> `size` | width of inner gaps for snapped windows. | `0`
`pad` <br> `[uint; 4]` | padding from screen edges for snapped windows, values are [Top, Bottom, Left, Right]. This sets the padding for the main monitor. If the bar is enabled its height is added to the top padding. | `[0, 0, 0, 0]`
`secondary_pad` <br> `[uint; 4]` | like `pad` but for all non-primary monitors. | `[0, 0, 0, 0]`
`smart_window_splacement` <br> `bool` | Try to place new clients so they don't overlap any existing clients or at least have as little overlap as possible. | `true`
`smart_window_placement_max` <br> `unint` | Do not attemp to do smart window placement if there are already this many clients on the main monitor. `0` means no limit. | `0`

### [window]

Key and type | Description | Default
---|---|---
`border` <br> `size` | size of border around windows | `1mm`
`title_font` <br> `font` | &nbsp; | `'sans'`
`title_font_size` <br> `uint` | see [Fonts](#fonts) | `14`
`title_font_scaling_percent` <br> `uint` | &nbsp; | `100`
`title_bar_height` <br> `size`, relative to title font size | height of the title bar | `1.1em`
`title_alignment` <br> `alignment` | window title alignment | `left`
`left_buttons` <br> `[string]` | left window buttons, valid values are `minimize`, `maximize`, and `close` | `[]`
`right_buttons` <br> `[string]` | &nbsp; | `['close']`
`icon_size` <br> `size`, relative to title bar height | size of window icons. If `0` window icons are disabled. | `80%`
`button_icon_size` <br> `size`, relative to title bar height | size of button icons | `75%`
`circle_buttons` <br> `bool` | enable [circular window buttons](#circular-buttons) | `false`
`extend_frame` <br>  `sizeo` | how far to extend the clickable window frame for resizing. `0` means disable frame extension. | `1mm`

### [theme]

Key and type | Description | Default
---|---|---
`colors` <br> `string` | name of the [color scheme](#color-scheme) | `'default'`
`icons` <br> `string` | name of the icon theme | `'Papirus'`

### [keys]

Key and type | Description | Default
---|---|---
`super` <br> `string` | the user-defined modifier. This is used for mouse key bindings as well as the `Mod` modifier in key bindings. | `'Super'`

### [keys.bindings]

See [Key bindings](#key-bindings).

### [bar]

Key and type | Description | Default
---|---|---
`height` <br> `size`, relative to font size | &nbsp; | `1.1em`
`font` <br> `font` | &nbsp; | `'sans 14'`
`time_format` <br> `string` | format for the date-time widget, uses strftime format. | `'%a %b %e %H:%M %Y'`
`localized_time` <br> `bool` | use the `LC_TIME` locale for the format values in `time_format` | `true`
`power_supply` <br> `string` | the power supply to use for the battery widget | `'BAT0'`
`update_interval` <br> `uint` | time between automatic bar updates in milliseconds | `10000`

### [dock]

Key and type | Description | Default
---|---|---
`height` <br> `size`, relative to main monitor height | &nbsp; | `10%`
`pinned` <br> `[string]` | pinned dock items, these are always in the dock even if they have no client | `[]`
`focused_client_on_top` <br> `bool` | move the focused instance of an application to the top of the list in context menus | `false`
`focused_urgent` <br> `bool` | if there is a urgent client, focus it instead of the active client when clicking a dock item | `false`
`item_size` <br> `size`, relative to dock height | size of dock items | `80%`
`icon_size` <dr> `size`, relative to item size | size of icons on items | `85%`
`context_show_workspaces` <br> `bool` | show the workspace of clients in context menus, unless all clients are on the current workspace | `true`
`auto_indicator_colors` | for programs with icons, use the average color of the icon as the indicator color. | `true`

### [split_handles]

Key and type | Description | Default
---|---|---
`size` <br> `size` | width of split handles | `2mm`
`vertical_sticky` <br> `[uint]` | percentages of the monitor width where split handles will stick to while moving | `[50]`
`horizontal_sticky` <br> `[uint]` | percentages of the monitor height where split handles will stick to while moving | `[50]`
`min_split_size` <br> `uint` | percentage of minimum split size based on monitor size | `10`

## Fonts

- `window.title_font_size`: this is the size the window title font uses on the primaty monitor, for all other monitors this is scaled to have the same visual size. Set to `0` to disable this feature and use the size specified in the `window.title_font` property.

- `window.tile_font_scaling_percent`: percentage that he title font scaling will take effect, since on some resolution and DPI combinations fonts may lose clarity at full scaling.

- `general.scale_base_fonts`: when set to `true`, the base size of all fonts is scaled to have the same visual size when the DPI of the primary monitor changes. This value is also affected by the `window.tile_font_scaling_percent` value.

## Key bindings

Example:

```ini
[keys]
[.bindings]
Mod+Shift+Q = quit
Mod+space = $ launcher
... = $ command 'one argument'
... = $ command "one 'argument'"
... = $ command "one \\"argument\\""
... = $ command one\ argument
```

In this section the keys are a `+`-separated list of 0 or more modifiers and a key.

The key names must adhere to the requirements of `XStringToKeysym`, that is:
```
Standard KeySym names are obtained from <X11/keysymdef.h> by removing the XK_ prefix from each name.
KeySyms that are not part of the Xlib standard also may be obtained with this function.
The set of KeySyms that are available in this manner and the mechanisms by which Xlib obtains them is implementation-dependent.
```

If a value starts with a `$` it's a command that should be run if the key is pressed.

Otherwise it must be one of the [actions](#actions).

## Actions

TODO, here are the names:

- `quit`
- `quit_dialog`
- `snap_left`
- `snap_right`
- `snap_up`
- `snap_down`
- `maximize`
- `unsnap_or_center`
- `close_window`
- `raise_all`
- `decrease_volume`
- `increase_volume`
- `mute_volume`
- `move_to_next_monitor`
- `move_to_prev_monitor`

## Color scheme

Color schemes are located at `colors/<name>.ini` relative to the config file.

Example:

```ini
[palette]
named_color = rgb(12, 34, 56)

[window]
focused = #456789
urgent = windows.buttons.close
#        Links to non-palette colors use full dotted paths

[bar]
background = named_color
#            Links to palette colors just use their names
```

All values are of type `color`.

### [palette]

This section is not used by anything and just contains colors to use in other sections.

If can contain any key.

### [misc]

Element | Description | Default
---|---|---
`background` | color of the root window if no other program sets the wallpaper | `#000000`

### [window]

Element | Description | Default
---|---|---
`focused`
`focused_text`
`normal`
`normal_text`
`selected`
`selected_text`
`urgent`
`urgent_text`

### [window.buttons]

Element | Description | Default
---|---|---
`close`
`close_hovered`
`maximize`
`maximize_hovered`
`minimize`
`minimize_hovered`

### [bar]

Element | Description | Default
---|---|---
`background`
`text`
`workspace`
`workspace_text`
`active_workspace`
`active_workspace_text`
`urgent_workspace`
`urgent_workspace_text`

### [dock]

Element | Description | Default
---|---|---
`background`
`hovered`
`urgent`
`indicator`

### [tooltip]

Element | Description | Default
---|---|---
`background`
`text`

### [context_menu]

Element | Description | Default
---|---|---
`background`
`text`

### [notifications]

Element | Description | Default
---|---|---
`background`
`text`
`divider`

## Circular buttons

If circle buttons are enabled, `<button>` becomes the circle color for normal windows and `<button>_hovered` becomes the circle color for focused windows. The color of the actual icon gets derived from the `<button>_hovered` color.
