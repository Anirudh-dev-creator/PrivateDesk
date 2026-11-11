# Feature status

Batch 1 (shell + Target Size Compressor) was built and tested on GitHub (green build).
Batch 2 (everything marked NEW) has **not** been compiled yet: the next GitHub build is its first test.

| Area | Feature | Status |
|---|---|---|
| Shell | Sidebar, light/dark/system theme | Built, confirmed |
| Compressor | JPEG/PNG output, 100 KB default, presets, KB/KiB/MB/MiB, maximum + closest, cancel, on-disk verification | Built, confirmed |
| Compressor | PDF (needs Ghostscript installed) | Built |
| **NEW** Batch Compress | Many files, one limit, output folder, suffix, retry failed, cancel, CSV report | Written, untested |
| **NEW** Image Studio | Crop, resize, rotate, flip, brightness, contrast, blur, sharpen, grayscale, sepia, JPEG/PNG/WebP(lossless)/BMP/TIFF output, JPEG DPI tag, EXIF viewer, before/after preview | Written, untested |
| **NEW** Passport Studio | Size presets (editable), exact mm × DPI output, zoom/move, background colour, print sheets (4×6, 5×7, A4, custom), cut guides, JPG/PNG export | Written, untested |
| **NEW** Utilities | SHA-256/MD5, QR code, Base64, text tools, JSON, duplicate finder (report only), bulk rename (preview + confirm) | Written, untested |

## Next batches (not built)
- **Batch 3 – PDF Tools:** merge, split, extract/delete/reorder/rotate pages, images to PDF, passport sheet as PDF.
- **Batch 4 – Windows integration:** right-click "PrivateDesk" menu, Send To, Open With, file associations.
- **Batch 5 – engines you install separately:** Word/Office conversion (LibreOffice), OCR (Tesseract), PDF to image.

## Not planned in this form / needs separate decisions
- Automatic face detection and AI background removal (needs an ONNX model file and extra libraries)
- Full Word-compatible document editor
- Scanner (WIA/TWAIN), local AI assistant, Hindi/Odia interface, encrypted vault
- DOCX/PPTX/XLSX size compression, WebP lossy output

## Known limits
- Compressor handles images up to 24 megapixels; Image and Passport tools up to 40 megapixels.
- Passport tools have no face detection: position the face manually and check official rules.
- PDF paths with non-English characters may fail with Ghostscript on Windows.
