# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A fork of **GNOME Papers**, branched at tag `50.2`, on branch
`fork/tabs-and-preview`. It adds tabs and a pinnable link preview, and restores
several Evince 46.3.1 ergonomics that Papers dropped. Upstream remains
`https://gitlab.gnome.org/GNOME/papers.git`.

See `FORK-PLAN.md` for why Papers rather than Evince, and `CHANGES.md` for
everything implemented.

## Build and run

Never build this natively: it needs GTK ≥ 4.17, libadwaita ≥ 1.8 and poppler
≥ 25.07, none of which Ubuntu 24.04 has. Always build in Flatpak.

```bash
flatpak-builder --user --install --force-clean --disable-rofiles-fuse \
    _flatpak build-aux/flatpak/org.gnome.Papers.Local.json
flatpak run --user org.gnome.Papers.Devel [file.pdf]
```

A full rebuild takes several minutes; poppler/exempi/djvulibre are cached after
the first one. The app id is `org.gnome.Papers.Devel`, not `.Local` — the
manifest filename kept its old name. **The flatpak app-id must equal the
application id compiled by the devel profile**, or single-instance registration
is refused by the D-Bus proxy and every launch becomes its own process.

## Architecture

Two languages, split by directory. This split decides where a change goes:

- **`libview/` is C** (`pps-*.c`). The view, rendering, link previews,
  selection, colours.
- **`shell/src/` is Rust** (40 files, no C). Window, document view, sidebars,
  preferences.
- **`shell/resources/*.blp`** are Blueprint templates compiled to `.ui`.
- **`libview/context/`** holds the model and context objects shared by both.

`PpsDocumentView` owns everything per-document — model, sidebars, and 61 of the
67 actions under a scoped `"doc"` action group. `PpsWindow` owns 6 window-level
actions and, in this fork, an `AdwTabView` of document views. This split is why
tabs were feasible; GTK routes `doc.*` to whichever tab holds focus.

The customizable shortcuts (`shell/src/shortcuts.rs`) fall out of that split:
window-scope entries become application accels (`set_accels_for_action`), but
document-scope ones must be a `GtkShortcutController` installed on each view,
because a `doc.*` action only resolves from a widget with that group in scope.
Both are rebuilt when the `custom-shortcuts` GSettings map changes.

## Testing

There is a GUI test rig, because most of this work is only observable on screen.
It runs the app on a **nested X display** so synthetic input never touches the
real desktop. It lives in the session scratchpad (`rig.py`); recreate it if the
scratchpad is gone — it is ~180 lines using `python-xlib` + XTEST + Pillow.

```python
from rig import Rig
r = Rig(); r.ensure_xephyr()
r.start("/path/doc.pdf")          # launches on :7, waits for the window
r.click(x, y, button)             # 1 left, 2 middle, 3 right
r.drag(x1, y1, x2, y2)
r.key("h", ctrl=True)
r.shot("out.png", box=(x, y, w, h))
```

The nested display has no DRI3, so it runs `GSK_RENDERER=cairo`. Consequences
worth knowing:

- **Timing-dependent bugs do not reproduce there.** The link-activation bug
  needed a compositor and the GL renderer; the rig never showed it.
- **Flicker and other temporal artifacts cannot be screenshotted.** Reason about
  the cause, then ask the user to judge.
- Window size is remembered in GSettings; a fresh app id starts at 600×600, and
  below 720sp the breakpoint collapses the sidebar, which looks like a bug.

## Things that will bite you

- **`pkill -f <pattern>`** matches Claude Code's own shell if the pattern
  appears in its command line. Use `flatpak kill org.gnome.Papers.Devel`.
- **Blueprint emits event controllers as `<child>`.** `GtkBox` treats a child as
  a widget to pack and silently drops the controller; `GtkSeparator` does not.
  Safer: add controllers from Rust with `add_controller()`.
- **`AdwOverlaySplitView` sizes its sidebar by a *fraction*** — it allocates
  `clamp(fraction * total, min, max)`. Converting a stored pixel width into a
  fraction is a trap: during initial allocation the split view is briefly a
  different size, so the result is wrong permanently, and per-tab conversion
  makes tabs disagree. Feed the pixel width in as `max-sidebar-width` and keep
  the fraction as a stored constant instead — no measuring, and the width
  becomes a ceiling rather than something that grows with the window.
- **A drag gesture on a widget that moves as a result of the drag oscillates.**
  Put it on a stationary ancestor and use absolute coordinates.
- **`pps_document_model_new()` and `new_with_document()` are broken upstream** —
  `annotation-model` is `CONSTRUCT_ONLY` with an unguarded `g_object_ref`, so any
  C caller gets a critical. Build models with `g_object_new(..., "annotation-model", …)`.
- **`PpsDocumentView::new()` is broken upstream** — it sets `application` and
  `show-menubar`, which the widget does not have. Use `glib::Object::builder()`.
- **`G_DEBUG=fatal-criticals` is set in the manifest**, so any critical aborts
  the app. That is useful, but a "crash" is often just a warning.

## Conventions

- Commit messages explain *why*, and record wrong hypotheses that were ruled out.
- Keep upstream-useful fixes in their own commits, separate from fork features,
  so the series stays rebaseable.
- Debug instrumentation is always removed before committing.
