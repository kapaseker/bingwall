# Bingwall

Bingwall selects, previews, and applies images from a user-selected wallpaper source as the desktop wallpaper for a Linux user.

## Language

**Current Wallpaper**:
The first valid entry in a Wallpaper Source's current Wallpaper Feed, regardless of whether its published date matches the user's local calendar date.
_Avoid_: Today's wallpaper, latest local wallpaper

**Wallpaper Source**:
A named provider from which Bingwall obtains a Wallpaper Feed. Bing and Spotlight are Wallpaper Sources.
_Avoid_: Provider, feed type

**Selected Wallpaper Source**:
The Wallpaper Source currently being browsed in the application. Changing it does not apply a wallpaper or change the Daily Change Source.
_Avoid_: Current source, active source

**Daily Change Source**:
The single Wallpaper Source assigned to Daily Change. Assigning another source replaces the previous assignment.
_Avoid_: Selected source, automatic source

**Wallpaper Entry**:
A dated image reference and its descriptive attribution parsed from the Wallpaper Feed.
_Avoid_: Post, record

**Wallpaper Preview**:
A reduced-resolution cached image used only to display a Wallpaper Entry in the application. It is never used as the Applied Wallpaper.
_Avoid_: Preview Wallpaper, cached wallpaper

**Selected Wallpaper**:
The Wallpaper Entry currently shown in the application's large preview. Selection alone never changes the desktop wallpaper.
_Avoid_: Current wallpaper, active wallpaper

**Applied Wallpaper**:
The image currently assigned to the user's desktop, whether chosen manually or by a Wallpaper Update.
_Avoid_: Selected wallpaper, preview

**Wallpaper Feed**:
An externally maintained, chronological collection of dated wallpaper entries supplied by one Wallpaper Source.
_Avoid_: Source file, wallpaper list

**Wallpaper Update**:
The unattended operation that refreshes the Wallpaper Feed and applies the Current Wallpaper when Daily Change is enabled, independently of the application window.
_Avoid_: UI refresh, sync

**Daily Change**:
The user-controlled opt-in setting that permits Bingwall to perform a Wallpaper Update from one Daily Change Source at the scheduled time. It is disabled until the user explicitly enables it.
_Avoid_: Auto-refresh, timer
