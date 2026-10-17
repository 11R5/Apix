# Packaging assets

Generated from `Logo/LogoFirst.jpg` by `cargo run -p icongen --release` (run from
the repository root). Do not edit the PNG, ICO or ICNS files by hand; change the
logo or `tools/icongen` and run the tool again.

| Path | Used by |
|---|---|
| `icons/apix-{16…1024}.png` | README, docs, anything that needs a plain PNG |
| `windows/apix.ico` | Embedded into `apix-desktop.exe` (`crates/apix-desktop/windows/apix.rc`) |
| `macos/Apix.icns` | The `.app` bundle (M9) |
| `linux/icons/hicolor/` | Installed to `/usr/share/icons/hicolor/` by packages (M9) |
| `linux/apix.desktop` | The desktop entry for menus and file associations |

Sizes of 32 px and below use a tighter crop of the bird and the X, so the mark stays readable.
