# Original Hungarian UI assets

Run `python -m tools.export_menu ../GYARI output` from the remake directory.
Pillow decodes the original PCX files; the existing DTX exporter preserves the
Mincho font atlas alpha. The exporter never writes into `GYARI`.

`output/retail_ui.json` is the runtime contract. Image paths are relative to
`output`. It includes:

- `main_background`: original 1024×768 red frame with baked Hungarian game title.
- `menu_title`: transparent Főmenü image and display dimensions.
- `buttons`: new, load, save, options, controls, display, audio, exit. Each contains
  `image`, `hover_image`, `width`, `height`, `x`, `y`, and the source localized text.
- `loading`: 28 lower-case world names mapped to `image`, `title`, `title_image`,
  `title_width`, `title_height`. Mappings and titles come from the retail game AI
  script and Hungarian text keys.
- `status.world` and `status.assets`: original localized loading-status text.
- `font`: original 512×512 Mincho atlas, 32×64 cells, character rectangles and advances.
- `source_images`: all 85 original menu and loading PCXs converted without
  changing their RGB pixels.

The 800×600 loading artwork already contains the large word Töltés. Display it
centered at 640×480 for the supplied reference layout, against black. Add only the
world title and status line separately. Do not duplicate the baked game title or
loading word.

Text PNGs compose original glyphs from `misc/fonts/Mincho/table.dtx` using the
CP1250 map in `table.txt`; this includes Hungarian double acute accents. No system
font, generated lettering, or screenshot is used. Text images keep native 64px
cell height; manifest dimensions request the display scale. Menu coordinates,
display sizes, fixed cell advance, and normal/hover tints are reconstruction
parameters, not independently recovered executable constants. Source provenance
is explicit so later visual comparisons can refine these without replacing art.

Verify with `python -m unittest discover -s tests -p test_menu_export.py`.
The test checks all 85 source conversions pixel-for-pixel, 28 loading mappings,
Hungarian text/glyph coverage, transparent text, and referenced loading assets.
