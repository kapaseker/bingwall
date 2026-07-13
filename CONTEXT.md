# Bingwall

Bingwall selects, previews, and applies daily Bing images as the desktop wallpaper for a Linux user.

## Language

**Current Wallpaper**:
The first valid entry in the Wallpaper Feed, regardless of whether its published date matches the user's local calendar date.
_Avoid_: Today's wallpaper, latest local wallpaper

**Wallpaper Entry**:
A dated image reference and its descriptive attribution parsed from the Wallpaper Feed.
_Avoid_: Post, record

**Selected Wallpaper**:
The Wallpaper Entry currently shown in the application's large preview. Selection alone never changes the desktop wallpaper.
_Avoid_: Current wallpaper, active wallpaper

**Applied Wallpaper**:
The image currently assigned to the user's desktop, whether chosen manually or by a Wallpaper Update.
_Avoid_: Selected wallpaper, preview

**Wallpaper Feed**:
The externally maintained, chronological collection of dated Bing wallpaper entries that Bingwall reads.
_Avoid_: Source file, wallpaper list

**Wallpaper Update**:
The unattended operation that refreshes the Wallpaper Feed and applies the Current Wallpaper when Daily Change is enabled, independently of the application window.
_Avoid_: UI refresh, sync

**Daily Change**:
The user-controlled opt-in setting that permits Bingwall to perform a Wallpaper Update at the scheduled time. It is disabled until the user explicitly enables it.
_Avoid_: Auto-refresh, timer
