# tty7 Design System

## 1. Atmosphere & Identity

tty7 is a quiet terminal command center: dense enough for repeated shell work,
but visually restrained so terminal output remains the main object. The
signature is editor-like chrome around live terminal panes: soft tonal surfaces,
compact icon controls, and stable panes that never reflow unexpectedly.

## 2. Color

Colors are runtime theme tokens derived in `src/ui/theme.rs` from the active
theme preset. New UI must use these semantic roles instead of fixed colors.

| Role | Token | Usage |
| --- | --- | --- |
| Window surface | `theme.background` | App body, terminal and preview bases |
| Text primary | `theme.foreground` | Active labels, body text |
| Text muted | `theme.muted_foreground` | Inactive labels, paths, hints |
| Border | `theme.border` | Panel dividers and subtle separators |
| Hover/selected fill | `theme.muted` | List hover rows and quiet selected rows |
| Raised fill | `theme.secondary` | Active tabs, active workspace chips |
| Sidebar surface | `theme.sidebar` / derived sidebar tokens | Persistent navigation columns |
| Accent | `theme.primary` / preset accent | Cursor, links, focused affordances only |

Rules:
- Keep the terminal canvas visually dominant.
- Use tonal shifts before borders; use borders only for panel boundaries.
- Do not add raw hex colors in UI code. Extend `presets.rs` or `theme.rs` first.

## 3. Typography

The terminal font is user-configurable and propagated through `Config`. UI text
uses GPUI component defaults plus the configured terminal font where content is
code-like.

| Level | GPUI Usage | Weight | Usage |
| --- | --- | --- | --- |
| Body | `text_sm` | regular | Tree rows, tab labels, settings rows |
| Caption | `text_xs` | regular/medium | Paths, metadata, secondary hints |
| Panel title | `text_sm` | medium | File tree root and compact panel labels |

Rules:
- Avoid display-scale type inside tool chrome.
- Labels truncate instead of wrapping in tabs, sidebars, and tree rows.
- Letter spacing stays default.

## 4. Spacing & Layout

Spacing follows GPUI's 4px-based helpers.

| Token | Value | Usage |
| --- | --- | --- |
| `px_1` / `gap_1` | 4px | Tight icon spacing |
| `px_2` / `gap_2` | 8px | Compact rows and icon buttons |
| `px_3` | 12px | Panel headers, tab chip padding |
| `px_4` | 16px | Preview body and larger headers |
| `h(px(26.))` | 26px | Dense file/workspace rows |
| `h(px(30.))` | 30px | Title bar icon tiles and tabs |
| `h(px(36.))` | 36px | Side panel headers |
| `h(px(40.))` | 40px | Title bar |

Rules:
- Fixed-format UI controls get stable widths/heights.
- Panes use `min_w_0` / `min_h_0` before scroll or truncation.
- Side panels are full-height bands, not floating cards.

## 5. Components

### Title Bar Tab Chip
- Structure: icon, truncating label or rename input, close/hint slot.
- States: active uses `secondary` + foreground; inactive uses muted text and
  muted hover; close affordance is revealed on hover.
- Interaction: click activates, double-click renames, drag reorders.

### Icon Tile
- Structure: 30px square, 15px icon, rounded 8px.
- States: muted default, muted-fill hover.
- Interaction: mouse down stops titlebar propagation.

### Side Rail Item
- Structure: fixed-width row or tile with icon and short label.
- States: active uses `secondary` + foreground; inactive uses muted text and
  muted hover.
- Interaction: click switches context; plus tile creates a new context.
- Workspace root follows the active terminal cwd, using project markers to keep
  the file tree on the nearest project root.

### File Tree Row
- Structure: chevron slot, file/folder icon, truncating name.
- States: selected uses `muted` + foreground; hover uses `muted`.
- Interaction: directories toggle expansion, files open or focus preview tabs.

### Preview Tab
- Structure: scrollable body; file identity stays in the tab strip and file tree.
- States: text, binary, and error bodies.
- Interaction: click focuses the tab body; one file maps to one preview tab.

## 6. Motion & Interaction

Motion is minimal and purposeful. Existing animation is limited to the home page
fade/cursor and GPUI component state transitions.

Rules:
- Hover and active states communicate affordance; do not add decorative motion.
- Pane and workspace switching should be immediate and stable.
- Mouse-hit areas in the title bar must call `occlude()` where needed so Windows
  titlebar dragging does not swallow clicks.

## 7. Depth & Surface

Strategy: tonal-shift with borders for structural panel edges.

| Surface | Treatment |
| --- | --- |
| App body | `background` |
| Active chip/item | `secondary` or `muted` |
| Hover row/item | `muted` |
| Panel edge | 1px `border` |
| Popover/elevated surface | `popover` |

Rules:
- Do not nest cards inside cards.
- Use 8px radius for reusable chrome unless the existing component fixes it.
- Sidebars and panels are bands attached to the window edge.
