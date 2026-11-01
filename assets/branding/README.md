# Nexdir brand guide

## Logo

**Concept A — "Next Directory" (official logo, approved by the project owner on 2026-10-30):** a folder whose body
carries a forward chevron. It reads as "go to the next directory", stays legible at 16 px,
and works on light and dark backgrounds thanks to its own gradient tile.

| File | Use |
|---|---|
| `svg/nexdir-logo.svg` | **Master.** App tile with gradient. All rasters are generated from it. |
| `svg/nexdir-mark.svg` | Symbol only, gradient fill, transparent background (docs, website). |
| `svg/nexdir-mark-mono.svg` | Symbol only, single colour (`currentColor`). |
| `svg/nexdir-social-preview.svg` / `png/nexdir-social-preview.png` | GitHub social preview, 1280×640. |
| `png/nexdir-logo-{16…1024}.png` | Raster sizes, incl. 512 and 1024. |
| `png/nexdir-discord-{512,1024}.png` | Discord server icon (full-bleed square; Discord crops to a circle — the symbol sits inside the safe circle). |
| `nexdir.ico` | Windows icon, 16–256 px. |
| `../../src-tauri/icons/` | Tauri icon set (generated with `tauri icon`). |

Alternative concepts kept for reference: `concepts/concept-b-tree.svg` (an "N" drawn as a
directory tree) and `concepts/concept-c-stack.svg` (stacked folders).

Regenerate everything after editing the master: `npm run branding`.

Rules: do not recolour, stretch, add effects or place the tile on busy backgrounds. Keep
clear space of at least 1/8 of the logo width around it.

## Colours

| Token | Light | Dark | Use |
|---|---|---|---|
| Brand indigo | `#5B5BF7` | — | Logo gradient start |
| Brand cyan | `#12B5D6` | — | Logo gradient end |
| Primary | `#4F4FE6` | `#8183FF` | Interactive accent, focus |
| Secondary | `#0E9FBD` | `#3CC8E4` | Secondary accent |
| Background | `#F6F7FB` | `#0F1220` | Window background |
| Surface | `#FFFFFF` | `#161A2C` | Content areas |
| Text | `#171A2B` | `#ECEEF8` | Primary text |
| Text muted | `#5A6080` | `#A3A9C7` | Secondary text |
| Border | `#DFE2EE` | `#2A3050` | Borders |
| Success / Warning / Danger | `#1A8F5A` / `#B7791F` / `#D0364B` | `#41C38A` / `#E3A84A` / `#F0677A` | Status |

The complete token set (spacing, radii, typography, icon sizes, elevation, states) lives in
[`src/styles/tokens.css`](../../src/styles/tokens.css).

## Typography

Segoe UI Variable (Windows 11) with Segoe UI fallback; Cascadia Code for paths and code.
Sizes: 11 / 12 / 13 (base) / 15 / 20 / 26 px.
