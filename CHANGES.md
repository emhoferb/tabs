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
| Open in a new window | `Shift+Ctrl+N` |
| Close tab (closes the window if it is the last) | `Ctrl+W` |
| Next / previous tab | `Ctrl+Tab` / `Ctrl+Shift+Tab`, `Ctrl+PgDn` / `Ctrl+PgUp` |

Documents opened from the file manager also become tabs. This required making
the application single-instance; see *Trade-offs*.

### Pinnable link preview

Hovering an internal link still shows the usual thumbnail. **Right-click it** and
the preview is pinned: the static thumbnail is replaced by a live view of the
destination that can be scrolled, drag-selected, and copied from with `Ctrl+C`.
Click outside to dismiss.

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

### Resizable sidebar

Drag the sidebar edge. The width is shared by every tab. The grab zone reaches
about 10px into the document side and barely into the sidebar, because a pointer
arrow is drawn down-right of its hotspot and a symmetric zone feels wrong coming
from the right.

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
11. **Closing a tab returned to the start page** while a document was still open.
12. **Outline sections collapsed** as soon as the reader scrolled past them.

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
| `sidebar-width` | 320 | Shared by all tabs |
| `highlight-style` | `outline` | `outline` or `fill` |
| `accent-source` | `system` | `system`, `upstream`, or a colour name |
| `keep-outline-expanded` | true | Sections stay open once opened |
| `open-in-new-tab` | true | Tabs rather than windows |
| `show-zoom-overlay` | true | Floating controls |
| `collapse-sidebar-when-narrow` | true | Auto-collapse below 720sp |

---

## Not done

- **GIFs in slides.** `GtkVideo` via `pps_view_put()` is the landing pad —
  upstream's own commit removing the GStreamer player says that is its purpose.
  Needs a slide deck to see what poppler reports for the embedded media.
- **Outline tracking while scrolling** is much improved but still driven by page
  number alone. Matching Evince would need the view's scroll offset within the
  page, which the sidebar cannot currently see.
