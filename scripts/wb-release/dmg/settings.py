# SPDX-License-Identifier: GPL-3.0-or-later
#
# dmgbuild settings for the Wispr Bro install window (OT2-E). build.sh passes
# every value with -D, read from src-tauri/tauri.fork.conf.json
# (bundle.macOS.dmg), so the layout has one source. dmgbuild writes the
# window's .DS_Store and the background alias itself (ds_store, mac_alias):
# no Finder, no AppleScript, nothing shown on screen; the image is attached
# with -nobrowse.
#
# `defines` is injected by dmgbuild.
import os.path

app = defines["app"]  # noqa: F821
app_name = os.path.basename(app)

format = "UDZO"
filesystem = "HFS+"
size = None
files = [app]
symlinks = {"Applications": "/Applications"}
icon = defines["volume_icon"]  # noqa: F821
# The ".app" extension is left visible: hiding it sets Finder info on the
# bundle (SetFile -a E), and `codesign --verify --strict` then rejects the
# app ("Finder information ... detritus not allowed").
hide_extensions = []

# The picture: background.png beside background@2x.png; dmgbuild joins them
# into one HiDPI TIFF (tiffutil -cathidpicheck) as .background.tiff.
background = defines["background"]  # noqa: F821

show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
default_view = "icon-view"
show_icon_preview = False
include_icon_view_settings = True
include_list_view_settings = False

window_rect = (
    (int(defines["window_x"]), int(defines["window_y"])),  # noqa: F821
    (int(defines["window_w"]), int(defines["window_h"])),  # noqa: F821
)
arrange_by = None
grid_offset = (0, 0)
grid_spacing = 100
label_pos = "bottom"
text_size = 13
icon_size = int(defines["icon_size"])  # noqa: F821

icon_locations = {
    app_name: (int(defines["app_x"]), int(defines["app_y"])),  # noqa: F821
    "Applications": (int(defines["apps_x"]), int(defines["apps_y"])),  # noqa: F821
}
