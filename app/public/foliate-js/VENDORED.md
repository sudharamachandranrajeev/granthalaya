# Vendored: foliate-js

- Source: https://github.com/johnfactotum/foliate-js
- Commit: 78914aef4466eb960965702401634c2cb348e9b1 (2026-05-01)
- Licence: MIT (`LICENSE`). Upstream says the library is not stable and recommends pinning a commit.
- Kept: the runtime modules the reader imports. Left out: the demo reader (`reader.*`, `ui/`), tests, build
  files and PDF.js source maps.
- `vendor/pdfjs/`: PDF.js 5.5.207 as shipped by foliate-js, Apache-2.0 (notice at the top of `pdf.mjs`); its
  standard fonts carry `LICENSE_FOXIT` and `LICENSE_LIBERATION`.
- `vendor/zip.js`: zip.js, BSD-3-Clause, https://github.com/gildas-lormeau/zip.js (Copyright (c) 2023, Gildas
  Lormeau). `vendor/fflate.js`: fflate, MIT, https://github.com/101arrowz/fflate (Copyright (c) Arjun Barrett). Both minified, as shipped by foliate-js.

These files are served as-is from `public/`, not bundled, because the PDF adapter loads its worker, fonts and
character maps by URL at run time. To update: copy the same files from a newer commit and change the commit above.
