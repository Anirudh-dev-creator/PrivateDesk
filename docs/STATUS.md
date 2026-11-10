# Feature status

Not compiled or run by the author: no Rust toolchain or Windows machine was available.
The GitHub workflow is the first real build and test.

| Feature | Status |
|---|---|
| App shell, sidebar, light/dark/system theme | Written, untested |
| Target Size Compressor: JPEG, PNG (WebP/BMP/TIFF as input) | Written, tests included |
| Presets 10 KB – 10 MB, custom size, KB/KiB/MB/MiB, exact byte limit shown | Written |
| Default 100 KB = 100,000 bytes | Written |
| Maximum mode and Closest-size mode (JPEG; both never exceed the limit) | Written, tests included |
| Output verified on disk; failed or cancelled jobs leave no file | Written, tests included |
| Cancel button | Written, untested in UI |
| PDF compression | Written; needs Ghostscript installed; tested earlier only as a Ghostscript command line |
| Minimum-quality slider, resize toggle, keep-transparency toggle | Written |

## Not built yet (and not shown as working)
- WebP **output** (the Rust image library only writes lossless WebP, which is usually larger than JPEG)
- DOCX / PPTX / XLSX compression
- Batch compression, before/after preview, DPI controls, real progress percentage
- Everything else in the master prompt: PDF tools, document editor, converter, image studio,
  passport photos, OCR, scanner, file manager, Explorer integration, local AI, installer options
  beyond the default NSIS setup, Hindi/Odia UI, Tailwind styling

## Known limits
- Images over 24 megapixels are rejected (memory safety on 4 GB PCs).
- PDF paths containing non-English characters may fail with Ghostscript on Windows.
- Closest mode may lower JPEG quality to land near the limit.
- No Cargo.lock / package-lock.json are included; they are created on the first build.
