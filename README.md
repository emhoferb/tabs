# <img src="data/icons/128x128/apps/org.gnome.Papers.Devel.png" width="48" alt=""> Papers with tabs

An unofficial fork of [GNOME Papers](https://gitlab.gnome.org/GNOME/papers)
50.2, the GNOME document viewer. It adds tabs and brings back a handful of
Evince habits that Papers dropped.

This is not a GNOME project, and GNOME has not endorsed it. Please report
problems with it here, not on the Papers issue tracker.

## What's different

- **Tabs** — documents open as tabs in one window. Drag a tab out to give it
  its own window, or onto another window's tab bar to move it there.
- **Pinnable link preview** — right-click a link's hover preview to pin it. It
  becomes a live view of the destination that you can scroll and select text in.
- **Animated slides** — slides made with the LaTeX `animate` package play, as
  they do in Acrobat and Okular.
- **Evince ergonomics** — a compact outline, wheel and touchpad page flipping
  in single-page view, a zoom readout with presets, and jumping to a page by
  typing its number. The sidebar also stops growing with the window.
- **Preferences** — highlight and selected-tab styles, an accent colour that
  follows Ubuntu's theme accents, and rebindable shortcuts for the fork's own
  actions.

[CHANGES.md](CHANGES.md) lists everything, with the reasoning behind each
change and every shortcut.

## Building

It only builds as a Flatpak. It needs GTK 4.17+, libadwaita 1.8 and a recent
poppler, and most distributions don't ship those yet.

```sh
flatpak install flathub org.gnome.Sdk//50 org.gnome.Platform//50 \
    org.freedesktop.Sdk.Extension.rust-stable//25.08
flatpak-builder --user --install --force-clean \
    _flatpak build-aux/flatpak/org.gnome.Papers.Local.json
flatpak run org.gnome.Papers.Devel [file.pdf]
```

The first build takes a while because it compiles poppler; later builds reuse
it. The build needs network access to fetch Rust crates. If FUSE is unavailable,
add `--disable-rofiles-fuse`.

It installs as `org.gnome.Papers.Devel`, alongside a regular Papers rather than
replacing it. GNOME's own nightly Papers build uses the same ID, so you can't
have both installed at once.

## Sandbox permissions

The same as upstream's development build. That includes read-only access to
your home folder and to removable media, which the built-in Open dialog uses to
browse them.

## License

GPL-2.0-or-later, like Papers; see [COPYING](COPYING). A few files inherited
from upstream are GPL-3.0-or-later, as marked in their headers.

All credit for Papers itself goes to its
[contributors](https://gitlab.gnome.org/GNOME/papers), and before them to
Evince's.
