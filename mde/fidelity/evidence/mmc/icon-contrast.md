# MMC icon contrast evidence

Rasterized with ImageMagick from installed Mackes themes.
Mean luminance of opaque pixels (0=black, 255=white).

| Icon | Theme role | Mean luminance | File |
|---|---|---:|---|
| `view-refresh` | light-glyphs | 244.2 | `icon-view-refresh-light-glyphs.png` |
| `view-refresh` | dark-glyphs | 0.0 | `icon-view-refresh-dark-glyphs.png` |
| `folder` | light-glyphs | 244.0 | `icon-folder-light-glyphs.png` |
| `folder` | dark-glyphs | 0.0 | `icon-folder-dark-glyphs.png` |
| `computer` | light-glyphs | 244.0 | `icon-computer-light-glyphs.png` |
| `computer` | dark-glyphs | 0.0 | `icon-computer-dark-glyphs.png` |
| `go-previous` | light-glyphs | 244.0 | `icon-go-previous-light-glyphs.png` |
| `go-previous` | dark-glyphs | 0.0 | `icon-go-previous-dark-glyphs.png` |
| `document-properties` | light-glyphs | 244.3 | `icon-document-properties-light-glyphs.png` |
| `document-properties` | dark-glyphs | 0.0 | `icon-document-properties-dark-glyphs.png` |

**Pass rule:** dark UI → light-glyphs (~#f4f4f4); light UI → dark-glyphs (near black).

Wired in code via `mde_theme::icons::for_ui(name, px, dark_ui)`.
