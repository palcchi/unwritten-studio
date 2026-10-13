# Unwritten Studio app icon

**Creature:** a unicorn, as an engraved head-and-shoulders bust looking out of the tile.

**Style:** an ink engraving portrait with a full-bleed colour field, tight head-and-shoulders crop,
and the body running off the bottom and right edges.

**Tile:** 512 × 512 viewBox, rounded square `rx=112`, clipped; no border or roundel.

**Palette** (exactly three colours):

| Colour | Hex | Used for |
|---|---|---|
| App colour (Unwritten Studio pink) | `#e0368f` | the full-bleed field |
| Paper | `#efe9dc` | the figure |
| Ink | `#0b0b0c` | line work and contour |

**Provenance:** the project owner created it as a 2880 px engraving-style image, kept in
craftrules at `assets/app-icons/unwritten-studio/source.png`). It was vectorised with craftrules
`assets/logo-options/_tools/vectorize_tile.py`, which keys out the field and traces the ink with potrace.
Licence: see [LICENSE.txt](LICENSE.txt) (MIT OR Apache-2.0, like the repo).

## Files

| File | What |
|---|---|
| `unwritten-studio.svg` | master vector, traced at 2048 px (canonical) |
| `unwritten-studio-small.svg` | lighter vector (~350 KB), traced at 1024 px; also the web favicon |
| `unwritten-studio-1024.png` | 1024 px render |
| `unwritten-studio-macos-512.png` | runtime Dock icon on macOS, on Apple's 824/1024 grid |
| `unwritten-studio.icns` | macOS icon (Apple grid) |
| `unwritten-studio.ico` | Windows icon, 16–256 px; embedded in `unwritten-studio.exe` by `apps/unwritten-studio/build.rs` |
| `hicolor/<n>x<n>/apps/com.unwrittenstudio.app.png` | Linux icon theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |
| `hicolor/scalable/apps/com.unwrittenstudio.app.svg` | Linux scalable icon |

App ID: `com.unwrittenstudio.app` (Wayland app ID, `packaging/linux/com.unwrittenstudio.app.desktop`).

## Regenerate

Copy a new `icon-master.svg` / `icon.svg` from craftrules `assets/app-icons/unwritten-studio/` over
`unwritten-studio.svg` / `unwritten-studio-small.svg`, then run `packaging/icons.sh` (needs `resvg`; `iconutil`
on macOS for the `.icns`). Copy `unwritten-studio-small.svg` to `apps/unwritten-studio-web/web/favicon.svg` too.
