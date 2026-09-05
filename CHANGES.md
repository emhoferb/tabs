# Changes in this fork

Everything this fork adds or fixes on top of GNOME Papers `50.2`, why it was
needed, and how to reach it.

---

## Features

### Tabs

Documents open as tabs in one window. The tab bar sits under the toolbar and
beside the sidebar, appearing once a second tab exists.

Neither Evince nor Papers ever had tabs. Papers made it feasible: it had already
split `PpsDocumentView` out of the window, with 61 of 67 actions in a scoped
`"doc"` action group, so GTK routes actions to the focused tab with no rewiring.

| Action | Shortcut |
| --- | --- |
| Open in a new tab | `Ctrl+T` |
| Open (tab or window, per preference) | `Ctrl+O` |
| Open a copy in a new tab | `Ctrl+Shift+D` |
| Open in a new window | `Ctrl+Shift+N` |
| Close tab (closes the window if it is the last) | `Ctrl+W` |
| Close window | `Ctrl+Shift+W` |
| Next / previous tab | `Ctrl+Tab` / `Ctrl+Shift+Tab`, `Ctrl+PgDn` / `Ctrl+PgUp` |

Opening a document into a tab is seamless: the current document stays on screen
while the new one loads behind a spinner on its own tab, which is revealed only
when ready, rather than the whole window flipping to a loading screen. The
outline loads immediately too — its links job used to sit at the lowest priority
behind every thumbnail render of the new tab.

Documents opened from the file manager also become tabs. This required making
the application single-instance; see *Trade-offs*.

### Pinnable link preview

Hovering an internal link still shows the usual thumbnail. **Right-click it** and
the preview is pinned: the static thumbnail is replaced by a live view of the
destination that can be scrolled, drag-selected, and copied from with `Ctrl+C`.
Click outside or press `Escape` to dismiss. The pinned preview no longer takes a
system keyboard grab, so `Alt+Tab` and other global shortcuts keep working while
it is up.

A thumbnail is a texture with no text layer, so nothing in it can ever be
selected. The live view is only built on an explicit pin — standing up a second
view with its own model and page cache is far too expensive to do on every hover.

### Middle-click a cross-reference

Opens its destination in a new tab, keeping the reference beside the text that
cited it.

### Zoom level display

A percentage readout in the header, opening the menu Evince had: Fit Page, Fit
Width, Automatic, and presets from 50% to 400%.

### Preferences dialog

From the main menu. Contains:

- **Highlight style** — coloured border (default) or filled
- **Accent colour** — follow the desktop, Papers' default blue, or any of the
  ten Ubuntu accents
- **Collapse sidebar in narrow windows** — off keeps the sidebar at all widths
- **Floating controls** — the zoom and annotation buttons over the page
- **Keep outline sections expanded**
- **Open documents in a new tab** — off restores one window per document
- **Customizable shortcuts** — a Shortcuts page rebinds the fork's own
  shortcuts (tabs, sidebar, outline, floating controls, preferences): activate a
  row, press the combination, `Backspace` to reset. Collisions within the set are
  refused. The accent chooser shows a coloured swatch for each option, including
  the resolved colour behind "Follow system".

### Two-finger and wheel page flipping

In non-continuous (single-page) view, scrolling past the edge of a page turns to
the next, and on a page that fits the window entirely a scroll turns the page
outright — the Evince behaviour Papers had dropped, where a fitted page simply
would not respond to the wheel or touchpad.

### Outline expand/collapse toggle

`F8`, or the floating button at the top of the outline sidebar, expands the
whole outline or collapses it back to just the top-level chapters — whichever
the current state calls for. The button's arrow shows which way it will go.

### Type a page number

Typing a bare digit over the document focuses the page box with that digit
already entered; finish the number and press Enter to jump. Digits were unused
as shortcuts, so nothing is lost.

### Resizable sidebar

Drag the sidebar edge. The width is shared by every tab. The grab zone reaches
about 10px into the document side and barely into the sidebar, because a pointer
arrow is drawn down-right of its hotspot and a symmetric zone feels wrong coming
from the right.

The dragged width is a **ceiling, not a target**. Widening the window never
widens the sidebar past it; narrowing the window past the point where the
sidebar would take up more than its share does narrow it, at libadwaita's
default rate of a quarter of the window. That is the Evince behaviour: the
sidebar has a settled width and only gives ground when there is no room for it.
Upstream Papers leaves the width purely proportional, so a wide window gets an
absurdly wide sidebar.

### Hideable floating controls

`Ctrl+H`, or the main menu. Hides the zoom *and* annotation buttons, so the page
is unobstructed. Applies to every tab and is remembered.

---

## Restored from Evince 46.3.1

- **Tighter outline.** Entries were wrapped over up to three lines with generous
  padding: about 14 fitted on screen where Evince fitted 45. Now one ellipsized
  line each, with the full text in the tooltip.
- **Search results** use the same metrics, so the two lists read as one family.
- **Accent colour** for selected text and the current entry in the outline,
  search results and thumbnails.

---

## Bugs fixed

### In Evince (found while evaluating it; fixed in the evince clone, not here)

1. **Hovering any internal link pinned a CPU core indefinitely.** The popover's
   motion handler unparented the popover from inside its own event dispatch,
   leaving an X11 pointer grab dangling and the display connection permanently
   readable: every `poll()` returned instantly with nothing draining it.
2. **The preview dismissed itself** as soon as it appeared, because an autohide
   popover grabs the pointer and pops up under the cursor.
3. **The popover stole the pointer** at the windowing-system level even with
   `can-target` off, killing the preview ~30ms after it became visible.
4. **`Ctrl`+scroll zoom did nothing.** The `GtkEventControllerScroll` had no
   flags, so it defaulted to `SCROLL_NONE` — "don't emit scroll" — and the
   handler was never called.

### In Papers (fixed here)

5. **Link activation often needed two clicks.** The hover popover was targetable
   and torn down inside press dispatch, so the press that should have followed
   the link was swallowed.
6. **Outline highlight skipped sections.** The page cache stored one entry per
   page, so a page with four subsections kept only the last.
7. **Clicking an outline entry jumped to a different one.** Navigation emits
   `page-changed` asynchronously behind a 200ms debounce, long after the guard
   against it lifts, so clicking 6.2.2 landed on 6.2.4.
8. **Zoom presets went to the wrong level.** `doc.zoom` takes a raw model scale,
   which only equals a percentage at 72 dpi.
9. **The sidebar was silently the wrong width.** A pixel width converted to a
   fraction during initial allocation is wrong permanently: 324px became 251px.
10. **Sidebar resizing wobbled.** The drag gesture lived on a widget that moved
    as a result of the drag, so a stationary pointer kept producing new offsets.
    Evince avoids this by using a `GtkPaned`, whose position is an exact pixel
    value in a stationary coordinate space.
11. **The sidebar grew without limit with the window**, since upstream sizes it
    as a plain fraction. The dragged width is now the maximum instead, with the
    fraction only governing how it shrinks in a window too narrow to hold it.
12. **A tab opened into a narrow window got its own sidebar width.** Deriving the
    fraction per tab meant measuring a layout still in flux; the fraction is now
    stored alongside the pixel width, so a new tab sets two properties and
    measures nothing.
13. **Closing a tab returned to the start page** while a document was still open.
14. **Outline sections collapsed** as soon as the reader scrolled past them.
15. **The floating annotation button reappeared on restart** even with the
    controls switched off: `open_document` forced it visible for any annotatable
    document, after the setting had already been applied.
16. **Opening a document into a tab reloaded the whole window** — it flipped to
    the loading screen instead of loading behind the new tab.
17. **The outline appeared seconds late in a new tab**: its links job ran at the
    lowest priority, behind every thumbnail render.
18. **The Ctrl+O picker was slow** — GtkFileDialog in Flatpak is a portal round
    trip. Open now uses an in-process chooser (~0.4s).
19. **The thumbnail selection jumped down then up** at the viewport edges,
    because `scroll_to(FOCUS|SELECT)` scrolled twice; it now scrolls minimally
    and only when the page is off-screen.
20. **Two-finger and wheel scrolling did nothing in single-page view** — a
    fitted page left the scrolled window nothing to scroll, so it never turned
    the page.
21. **A pinned link preview swallowed Alt+Tab**: its autohide popover took a
    system keyboard grab. It no longer uses autohide.
22. **The pinned preview outlived its welcome** — it ignored Escape unless
    focus was inside it, ignored clicks on the sidebar or header bar, floated
    above other applications after Alt+Tab, and passed edge scrolls through to
    the document behind it. Dismissal now lives at the window level (outside
    clicks, focus loss), Escape asks libview first, and a scroll barrier on
    the popover keeps its scrolling inside.
23. **The Ctrl+O chooser lost the user's bookmarks**: the sandbox's config dir
    hides them, so they are read from the home mount and added as shortcut
    folders.
24. **The accent swatch lagged one choice behind** and "Follow system" showed
    the applied colour rather than the system's.
25. **Changing the highlight style needed a restart** — the settings instance
    holding the subscription was dropped immediately.

### Upstream bugs worth reporting

- `pps_document_model_new()` and `pps_document_model_new_with_document()` both
  emit `g_object_ref: assertion 'G_IS_OBJECT (object)' failed` for any C caller:
  `annotation-model` is `CONSTRUCT_ONLY` and its setter refs without a NULL
  check (`libview/context/pps-document-model.c:141`). Unnoticed because Papers
  only ever builds models from Blueprint.
- `PpsDocumentView::new()` sets `application` and `show-menubar`, properties the
  widget does not have, so constructing through it panics. Nothing calls it.

---

## Trade-offs taken

**Process isolation.** Upstream gives every document its own process, on purpose
(`NON_UNIQUE`, and spawning for a second document). Tabs are incompatible with
that, and a file-manager launch cannot otherwise reach a running window. The
application is now single-instance, so documents opened together share a
process. Set *Open documents in a new tab* off to get windows back.

**Accent colour is inferred.** Ubuntu 24.04 publishes no accent through the
portal — `org.freedesktop.appearance accent-color` does not exist before GNOME
47. It selects an accent by swapping GTK theme variants (`Yaru-red-dark`), so the
accent is read from the theme name via the settings portal, and only when
libadwaita reports its default blue. A desktop that does publish an accent keeps
precedence, so this resolves itself on a newer system.

---

## Settings

All under `org.gnome.Papers`:

| Key | Default | Meaning |
| --- | --- | --- |
| `sidebar-width` | 320 | Shared by all tabs; a ceiling, not a fixed width |
| `sidebar-width-fraction` | 0.25 | Rate the sidebar narrows at once the window is too small for `sidebar-width` |
| `highlight-style` | `outline` | `outline` or `fill` |
| `accent-source` | `system` | `system`, `upstream`, or a colour name |
| `keep-outline-expanded` | true | Sections stay open once opened |
| `open-in-new-tab` | true | Tabs rather than windows |
| `show-zoom-overlay` | true | Floating controls |
| `collapse-sidebar-when-narrow` | true | Auto-collapse below 720sp |
| `custom-shortcuts` | `{}` | Per-action shortcut overrides (id → accel) |

---

## Not done

- **GIFs in slides.** `GtkVideo` via `pps_view_put()` is the landing pad —
  upstream's own commit removing the GStreamer player says that is its purpose.
  Needs a slide deck to see what poppler reports for the embedded media.
- **Outline tracking while scrolling** is much improved but still driven by page
  number alone. Matching Evince would need the view's scroll offset within the
  page, which the sidebar cannot currently see.
