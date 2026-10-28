# Baka Launcher

A small Linux application launcher built with iced master. Running the executable
opens the launcher with search already focused. Type to calculate or filter
installed apps. For app results, press **Enter** to open the first match or click
an app. **Escape** closes the
window. The launcher closes after the OS accepts an application launch; an error
keeps the window open so you can search again.

Queries are evaluated with fasteval first: `2 + 3 * 4` displays `14`. Press
**Enter** or click the calculation result to copy its value to the clipboard.
The window stays open and confirms the copy; clipboard failures show an error.
If evaluation fails, the query uses the normal application search instead.
Arithmetic uses floating-point numbers and supports parentheses, powers (`^`),
remainder (`%`), and functions such as `sin` and `log`.

**Right-click an application** and select **Pin to Pinned** or **Unpin from
Pinned**. The change appears after saving and persists between launches. Click
outside the menu or press Escape to dismiss it; Escape again closes the launcher.
This works in both the application grid and search results.

The window has fixed size limits so tiling compositors such as COSMIC treat it
as a floating dialog rather than adding it to the tiled layout.

The dark layout follows the supplied reference: search, pinned applications,
application grid, and a narrow rail. Refresh rescans installed applications;
the exit control closes the launcher. The moon indicates dark appearance and
the disabled power icon does not perform a session action.

## Build and run

```sh
cargo build --release --locked
./target/release/baka-launcher
```

Copy `target/release/baka-launcher` to a compatible Linux system and run it.
There is no installer, app-owned sidecar resource, service, or required launcher
helper. Fira Sans and fallback UI artwork are embedded. Installed applications
and their icons are read from the host desktop. Normal Linux C runtime, display
server (X11 or Wayland), and graphics driver infrastructure are required.

Iced's **wgpu hardware renderer** is enabled, with **tiny-skia software fallback**.
Both iced and iced_test use the master branch, locked to the same revision in
Cargo.lock. Use `cargo update` deliberately to move that revision.

## Application discovery

Each invocation scans `$XDG_DATA_HOME/applications` (default
`~/.local/share/applications`) and `$XDG_DATA_DIRS` (default
`/usr/local/share:/usr/share`). No cache or background service remains running.
User desktop entries override system entries, including hidden overrides.
Desktop visibility, localized names, keywords, and TryExec are respected.

Installed Chrome, Brave, VS Code, Files, and Obsidian entries are default pins.
Your pin/unpin overrides are saved in `$XDG_CONFIG_HOME/baka-launcher/pins.conf`
(default `~/.config/baka-launcher/pins.conf`). Each line is `+desktop-id` to pin or
`-desktop-id` to unpin; remove an override to restore that application's default.
The file is created only when a preference changes and is not required to run.
If saving fails, the launcher shows an error and keeps the previous pin choices.
Missing applications are never replaced with fake launchable entries. An empty
search shows pins first and the remaining apps alphabetically. Typed search
ranks exact names, prefixes, substrings, then metadata matches, independently
of pin status. Enter uses the first entry in that displayed order.

Commands are parsed from desktop entries into an argument vector and launched
directly, without shell interpretation. File/URL placeholders are omitted for
ordinary launches. Terminal applications require an already installed supported
terminal: x-terminal-emulator, cosmic-term, kitty, alacritty, foot, wezterm,
gnome-terminal, kgx, konsole, xfce4-terminal, or xterm. Activation-only D-Bus
entries without an Exec fallback are not currently supported. Native discovery
on Windows and macOS is not implemented.

Common SVG/PNG icon locations are searched with bounded file sizes. Missing or
unreadable icons use the embedded fallback. Icon data is captured during the
scan; rendering does not require the original icon file to remain present.

## Verification

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --locked
ICED_TEST_BACKEND=tiny-skia cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
ICED_TEST_BACKEND=wgpu cargo test --lib launcher::tests::render_reference_preview
```

GUI tests run headlessly, applying the actual boot focus task to the widget
tree, typing without clicking, and checking Enter, errors, geometry, scrolling,
right-click menus, and persistent pin/unpin changes.
The preview test writes `target/launcher-preview-tiny-skia.png` or
`target/launcher-preview-wgpu.png`. Preview fixture names match the reference;
icons reflect which of those applications are installed on the test machine.

The MAP plan, review resolutions, phase documents, resume configuration and
handoff live in `.claude/map/iced-launcher/`.

Iced is MIT licensed. Its embedded Fira Sans font is licensed under the SIL Open
Font License; font licensing information is available from the
[Fira project](https://github.com/mozilla/Fira).
