//! Local target-size compressor for PrivateDesk.
//!
//! Images: JPEG, PNG, WebP, BMP, TIFF in; JPEG or PNG out.
//! PDF: local Ghostscript `pdfwrite`. Ghostscript must be installed or bundled.
//! No network access is used anywhere in this crate.
//!
//! Size rule: a limit of N bytes means the saved file is <= N bytes. Files are
//! never padded. Every result is re-measured on disk before success is reported.

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder, ImageReader};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tempfile::NamedTempFile;

pub const MIN_TARGET_BYTES: u64 = 1000;
const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 24_000_000;
const MAX_JPEG_QUALITY: u8 = 95;
const MIN_SIDE_PIXELS: u32 = 48;

static CANCEL: AtomicBool = AtomicBool::new(false);

pub fn request_cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}

pub fn reset_cancel() {
    CANCEL.store(false, Ordering::Relaxed);
}

fn check_cancel() -> Result<(), String> {
    if CANCEL.load(Ordering::Relaxed) {
        Err("Compression was cancelled. No file was saved.".into())
    } else {
        Ok(())
    }
}

fn default_mode() -> String {
    "maximum".into()
}

fn default_min_quality() -> u8 {
    20
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionRequest {
    pub input_path: String,
    pub output_path: String,
    pub target_bytes: u64,
    /// "auto", "jpeg" or "png". PDF input always produces PDF.
    pub output_format: String,
    /// "maximum" (default) or "closest". Both guarantee output <= target_bytes.
    #[serde(default = "default_mode")]
    pub mode: String,
    pub allow_resize: bool,
    /// If true, "auto" keeps transparent images as PNG.
    pub preserve_transparency: bool,
    /// Lowest JPEG quality the engine may use (1-95).
    #[serde(default = "default_min_quality")]
    pub min_quality: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionResult {
    pub input_path: String,
    pub output_path: String,
    pub original_bytes: u64,
    pub output_bytes: u64,
    pub target_bytes: u64,
    pub reduction_percent: f64,
    pub output_format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quality: Option<u8>,
    pub mode: String,
    pub met_target: bool,
    pub note: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressorFileInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub extension: String,
    pub is_pdf: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_alpha: Option<bool>,
}

struct Encoded {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    quality: Option<u8>,
}

pub fn ghostscript_available() -> bool {
    find_ghostscript().is_some()
}

pub fn inspect_file(input_path: &str) -> Result<CompressorFileInfo, String> {
    let path = PathBuf::from(input_path);
    let metadata = fs::metadata(&path).map_err(|e| format!("Cannot read file: {e}"))?;
    if !metadata.is_file() {
        return Err("Please select a file, not a folder.".into());
    }
    if metadata.len() > MAX_INPUT_BYTES {
        return Err("The selected file exceeds the 512 MB safety limit.".into());
    }
    let ext = extension_lower(&path);
    let is_pdf = has_pdf_signature(&path)?;
    let (width, height, has_alpha) = if is_pdf {
        (None, None, None)
    } else {
        ensure_supported_image_ext(&ext)?;
        let image = read_image(&path)?;
        let (w, h) = image.dimensions();
        (Some(w), Some(h), Some(image.color().has_alpha()))
    };
    Ok(CompressorFileInfo {
        path: input_path.to_string(),
        name: path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Selected file")
            .to_string(),
        size_bytes: metadata.len(),
        extension: ext,
        is_pdf,
        width,
        height,
        has_alpha,
    })
}

pub fn compress(request: CompressionRequest) -> Result<CompressionResult, String> {
    if request.target_bytes < MIN_TARGET_BYTES {
        return Err("Target size must be at least 1 KB (1,000 bytes).".into());
    }
    let closest = match request.mode.to_ascii_lowercase().as_str() {
        "maximum" => false,
        "closest" => true,
        _ => return Err("Unknown compression mode.".into()),
    };
    let mode_name: String = (if closest { "closest" } else { "maximum" }).to_string();
    let min_quality = request.min_quality.clamp(1, MAX_JPEG_QUALITY);
    let target = request.target_bytes;

    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_path);
    let input_meta = fs::metadata(&input).map_err(|e| format!("Cannot read input file: {e}"))?;
    if !input_meta.is_file() {
        return Err("Input path is not a regular file.".into());
    }
    if input_meta.len() > MAX_INPUT_BYTES {
        return Err("The selected file exceeds the 512 MB safety limit.".into());
    }
    if same_existing_file(&input, &output)? {
        return Err("The output must be a different file. The original is never overwritten.".into());
    }
    if output.exists() {
        return Err("That output file already exists. Choose a new filename to protect existing files.".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err("The output folder does not exist.".into());
    }

    let ext = extension_lower(&input);
    let is_pdf = has_pdf_signature(&input)?;
    check_cancel()?;

    // Auto mode: a file already within the limit is copied unchanged.
    if request.output_format.eq_ignore_ascii_case("auto") && input_meta.len() <= target {
        if !is_pdf {
            ensure_supported_image_ext(&ext)?;
        }
        copy_new_file_atomically(&input, &output)?;
        let copied = fs::metadata(&output)
            .map_err(|e| format!("Cannot verify copied output size: {e}"))?
            .len();
        if copied != input_meta.len() || copied > target {
            let _ = fs::remove_file(&output);
            return Err("Final copy verification failed. The incomplete output was removed.".into());
        }
        let format = if is_pdf {
            "pdf".to_string()
        } else if ext == "jpg" {
            "jpeg".to_string()
        } else {
            ext.clone()
        };
        return Ok(CompressionResult {
            input_path: request.input_path,
            output_path: request.output_path,
            original_bytes: input_meta.len(),
            output_bytes: copied,
            target_bytes: target,
            reduction_percent: 0.0,
            output_format: format,
            width: None,
            height: None,
            quality: None,
            mode: mode_name,
            met_target: true,
            note: "The original file was already within the maximum size, so it was copied unchanged. No recompression was needed.".into(),
        });
    }

    if is_pdf {
        return compress_pdf(&input, &output, target, input_meta.len(), mode_name);
    }

    ensure_supported_image_ext(&ext)?;
    let original = read_image(&input)?;
    let (width, height) = original.dimensions();
    let target_format = choose_image_format(
        &request.output_format,
        original.color().has_alpha(),
        request.preserve_transparency,
    )?;
    let encoded = match target_format.as_str() {
        "jpeg" => compress_image_jpeg(&original, target, request.allow_resize, min_quality, closest)?,
        "png" => compress_image_png(&original, target, request.allow_resize)?,
        _ => return Err("Unsupported output format. Choose Auto, JPEG or PNG.".into()),
    };
    if encoded.bytes.len() as u64 > target {
        return Err("Compression engine returned a file above the target limit; output was not saved.".into());
    }
    check_cancel()?;
    write_new_file_atomically(&output, &encoded.bytes)?;
    let output_bytes = fs::metadata(&output)
        .map_err(|e| format!("Cannot verify output size: {e}"))?
        .len();
    if output_bytes > target || output_bytes != encoded.bytes.len() as u64 {
        let _ = fs::remove_file(&output);
        return Err("Final file-size verification failed. The incomplete output was removed.".into());
    }

    let resized = encoded.width != width || encoded.height != height;
    Ok(CompressionResult {
        input_path: request.input_path,
        output_path: request.output_path,
        original_bytes: input_meta.len(),
        output_bytes,
        target_bytes: target,
        reduction_percent: reduction(input_meta.len(), output_bytes),
        output_format: target_format,
        width: Some(encoded.width),
        height: Some(encoded.height),
        quality: encoded.quality,
        mode: mode_name,
        met_target: output_bytes <= target,
        note: if resized {
            "The image was resized to satisfy the size limit. The original file was left unchanged.".into()
        } else {
            "The image was re-encoded locally at its original dimensions. The original file was left unchanged.".into()
        },
    })
}

fn read_image(path: &Path) -> Result<DynamicImage, String> {
    // Check the header first so oversized images are rejected before decoding.
    let (w, h) = image::image_dimensions(path).map_err(|e| format!("Cannot read image dimensions: {e}"))?;
    ensure_safe_dimensions(w, h)?;
    ImageReader::open(path)
        .map_err(|e| format!("Cannot open image: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Cannot identify image format: {e}"))?
        .decode()
        .map_err(|e| format!("Image decode failed: {e}"))
}

fn ensure_supported_image_ext(ext: &str) -> Result<(), String> {
    match ext {
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff" => Ok(()),
        _ => Err(format!(
            "Unsupported file type: .{ext}. Supported inputs are JPG, PNG, WebP, BMP, TIFF and PDF."
        )),
    }
}

fn ensure_safe_dimensions(width: u32, height: u32) -> Result<(), String> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0 || height == 0 || pixels > MAX_IMAGE_PIXELS {
        return Err("Image dimensions are invalid or exceed the 24-megapixel safety limit.".into());
    }
    Ok(())
}

pub fn choose_image_format(requested: &str, has_alpha: bool, preserve_transparency: bool) -> Result<String, String> {
    match requested.to_ascii_lowercase().as_str() {
        "auto" => Ok((if has_alpha && preserve_transparency { "png" } else { "jpeg" }).to_string()),
        "jpeg" | "jpg" => Ok("jpeg".into()),
        "png" => Ok("png".into()),
        _ => Err("Unknown image output format.".into()),
    }
}

fn scaled_dims(w: u32, h: u32, scale: f64) -> (u32, u32) {
    (
        ((f64::from(w) * scale).round() as u32).max(1),
        ((f64::from(h) * scale).round() as u32).max(1),
    )
}

fn flatten_on_white(image: &DynamicImage) -> image::RgbImage {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    image::RgbImage::from_fn(w, h, |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = u16::from(p[3]);
        let blend = |c: u8| -> u8 { ((u16::from(c) * a + 255 * (255 - a) + 127) / 255) as u8 };
        image::Rgb([blend(p[0]), blend(p[1]), blend(p[2])])
    })
}

/// Highest JPEG quality (>= min_q) whose encoded size fits, at the given dimensions.
fn jpeg_best_at(image: &DynamicImage, w: u32, h: u32, target: u64, min_q: u8) -> Result<Option<Encoded>, String> {
    check_cancel()?;
    let (ow, oh) = image.dimensions();
    let rgb = if w == ow && h == oh {
        flatten_on_white(image)
    } else {
        flatten_on_white(&image.resize_exact(w, h, image::imageops::FilterType::Lanczos3))
    };
    let mut low = i16::from(min_q);
    let mut high = i16::from(MAX_JPEG_QUALITY);
    let mut best: Option<Encoded> = None;
    while low <= high {
        check_cancel()?;
        let quality = ((low + high) / 2) as u8;
        let mut candidate = Vec::new();
        JpegEncoder::new_with_quality(&mut candidate, quality)
            .write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
            .map_err(|e| format!("JPEG encoding failed: {e}"))?;
        if candidate.len() as u64 <= target {
            best = Some(Encoded { bytes: candidate, width: w, height: h, quality: Some(quality) });
            low = i16::from(quality) + 1;
        } else {
            high = i16::from(quality) - 1;
        }
    }
    Ok(best)
}

fn compress_image_jpeg(
    image: &DynamicImage,
    target: u64,
    allow_resize: bool,
    min_q: u8,
    closest: bool,
) -> Result<Encoded, String> {
    let (ow, oh) = image.dimensions();
    if let Some(found) = jpeg_best_at(image, ow, oh, target, min_q)? {
        return Ok(found);
    }
    if !allow_resize {
        return Err("The JPEG cannot fit within the selected size at the lowest allowed quality. Increase the target, lower the minimum quality, or allow resolution reduction.".into());
    }

    let mut prev_scale = 1.0_f64;
    let mut scale = 1.0_f64;
    let mut fit: Option<(Encoded, f64)> = None;
    for _ in 0..40 {
        prev_scale = scale;
        scale *= 0.88;
        let (w, h) = scaled_dims(ow, oh, scale);
        if let Some(found) = jpeg_best_at(image, w, h, target, min_q)? {
            fit = Some((found, scale));
            break;
        }
        if w <= MIN_SIDE_PIXELS && h <= MIN_SIDE_PIXELS {
            break;
        }
    }
    let (mut best, fit_scale) = fit.ok_or_else(|| {
        "Target size cannot be achieved while maintaining acceptable quality. Try a larger target size or allow resolution reduction.".to_string()
    })?;

    if closest {
        // Search between the last scale that fit and the one that did not, keeping the
        // largest valid file (never above the limit).
        let mut lo = fit_scale;
        let mut hi = prev_scale;
        for _ in 0..6 {
            let mid = (lo + hi) / 2.0;
            let (w, h) = scaled_dims(ow, oh, mid);
            match jpeg_best_at(image, w, h, target, min_q)? {
                Some(candidate) => {
                    lo = mid;
                    if candidate.bytes.len() > best.bytes.len() {
                        best = candidate;
                    }
                }
                None => hi = mid,
            }
        }
    }
    Ok(best)
}

fn compress_image_png(image: &DynamicImage, target: u64, allow_resize: bool) -> Result<Encoded, String> {
    let (ow, oh) = image.dimensions();
    let filters = [
        FilterType::Adaptive,
        FilterType::Paeth,
        FilterType::Sub,
        FilterType::Up,
        FilterType::Avg,
        FilterType::NoFilter,
    ];
    let attempts = if allow_resize { 20 } else { 1 };
    let mut scale = 1.0_f64;
    for _ in 0..attempts {
        check_cancel()?;
        let (w, h) = scaled_dims(ow, oh, scale);
        let rgba = if w == ow && h == oh {
            image.to_rgba8()
        } else {
            image.resize_exact(w, h, image::imageops::FilterType::Lanczos3).to_rgba8()
        };
        let mut best: Option<Vec<u8>> = None;
        for filter in filters {
            check_cancel()?;
            let mut candidate = Vec::new();
            PngEncoder::new_with_quality(&mut candidate, CompressionType::Best, filter)
                .write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(|e| format!("PNG encoding failed: {e}"))?;
            if candidate.len() as u64 <= target && best.as_ref().map(|b| candidate.len() < b.len()).unwrap_or(true) {
                best = Some(candidate);
            }
        }
        if let Some(bytes) = best {
            return Ok(Encoded { bytes, width: w, height: h, quality: None });
        }
        if !allow_resize || (w <= MIN_SIDE_PIXELS && h <= MIN_SIDE_PIXELS) {
            break;
        }
        scale *= 0.88;
    }
    Err("Lossless PNG compression cannot reach this target. Choose JPEG (transparency will be flattened to white) or increase the target size.".into())
}

fn compress_pdf(
    input: &Path,
    output: &Path,
    target: u64,
    original_bytes: u64,
    mode_name: String,
) -> Result<CompressionResult, String> {
    let gs = find_ghostscript().ok_or_else(|| {
        "PDF compression needs the free Ghostscript program installed on this PC (no online service is used). Install Ghostscript, then try again.".to_string()
    })?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp_dir = tempfile::Builder::new()
        .prefix("privatedesk-pdf-")
        .tempdir_in(parent)
        .map_err(|e| format!("Cannot create local temporary folder: {e}"))?;

    let resolutions = [300_u32, 220, 180, 150, 120, 96, 72, 60, 48, 36];
    let mut best: Option<(PathBuf, u64)> = None;
    for (index, dpi) in resolutions.iter().enumerate() {
        check_cancel()?;
        let candidate = temp_dir.path().join(format!("candidate_{index}.pdf"));
        let result = Command::new(&gs)
            .arg("-dSAFER")
            .arg("-sDEVICE=pdfwrite")
            .arg("-dCompatibilityLevel=1.4")
            .arg("-dPDFSETTINGS=/ebook")
            .arg("-dDownsampleColorImages=true")
            .arg("-dColorImageDownsampleType=/Bicubic")
            .arg(format!("-dColorImageResolution={dpi}"))
            .arg("-dDownsampleGrayImages=true")
            .arg("-dGrayImageDownsampleType=/Bicubic")
            .arg(format!("-dGrayImageResolution={dpi}"))
            .arg("-dDownsampleMonoImages=true")
            .arg(format!("-dMonoImageResolution={}", dpi.saturating_mul(2)))
            .arg("-dDetectDuplicateImages=true")
            .arg("-dCompressFonts=true")
            .arg("-dSubsetFonts=true")
            .arg("-dNOPAUSE")
            .arg("-dBATCH")
            .arg("-dQUIET")
            .arg(format!("-sOutputFile={}", candidate.to_string_lossy()))
            .arg(input)
            .output()
            .map_err(|e| format!("Could not start Ghostscript: {e}"))?;
        if !result.status.success() || !candidate.exists() {
            continue;
        }
        let size = fs::metadata(&candidate)
            .map_err(|e| format!("Cannot inspect PDF candidate: {e}"))?
            .len();
        if size >= 5
            && has_pdf_signature(&candidate)?
            && size <= target
            && best.as_ref().map(|(_, old)| size > *old).unwrap_or(true)
        {
            best = Some((candidate, size));
        }
    }

    let (best_path, candidate_bytes) = best.ok_or_else(|| {
        "The PDF could not be reduced below the requested limit without changing it too much. Try a larger target. The source PDF was not changed.".to_string()
    })?;
    check_cancel()?;
    copy_new_file_atomically(&best_path, output)?;
    let verified = fs::metadata(output)
        .map_err(|e| format!("Cannot verify PDF output: {e}"))?
        .len();
    if verified > target || verified != candidate_bytes {
        let _ = fs::remove_file(output);
        return Err("Final PDF byte-count verification failed. The incomplete output was removed.".into());
    }
    Ok(CompressionResult {
        input_path: input.to_string_lossy().into_owned(),
        output_path: output.to_string_lossy().into_owned(),
        original_bytes,
        output_bytes: verified,
        target_bytes: target,
        reduction_percent: reduction(original_bytes, verified),
        output_format: "pdf".into(),
        width: None,
        height: None,
        quality: None,
        mode: mode_name,
        met_target: verified <= target,
        note: "The PDF was re-written locally by Ghostscript with progressively lower image resolution. Text and vector content is normally kept, but image quality may drop.".into(),
    })
}

fn find_ghostscript() -> Option<PathBuf> {
    if let Ok(configured) = std::env::var("PRIVATEDESK_GHOSTSCRIPT") {
        let p = PathBuf::from(configured);
        if p.is_file() {
            return Some(p);
        }
    }
    let names: &[&str] = if cfg!(windows) { &["gswin64c.exe", "gswin32c.exe", "gs.exe"] } else { &["gs"] };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(base) = exe.parent() {
            for relative in ["resources/ghostscript/bin", "ghostscript/bin", "resources"] {
                for name in names {
                    let candidate = base.join(relative).join(name);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for folder in std::env::split_paths(&path_var) {
            for name in names {
                let candidate = folder.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    // Default Windows install location: C:\Program Files\gs\gs<version>\bin
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(var) {
                let gs_dir = PathBuf::from(pf).join("gs");
                if let Ok(entries) = fs::read_dir(&gs_dir) {
                    let mut versions: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
                    versions.sort();
                    versions.reverse();
                    for v in versions {
                        for name in names {
                            let candidate = v.join("bin").join(name);
                            if candidate.is_file() {
                                return Some(candidate);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn has_pdf_signature(path: &Path) -> Result<bool, String> {
    let mut file = File::open(path).map_err(|e| format!("Cannot read file header: {e}"))?;
    let mut header = [0_u8; 5];
    let count = file.read(&mut header).map_err(|e| format!("Cannot read file header: {e}"))?;
    Ok(count == 5 && &header == b"%PDF-")
}

fn extension_lower(path: &Path) -> String {
    path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn same_existing_file(input: &Path, output: &Path) -> Result<bool, String> {
    if !output.exists() {
        return Ok(false);
    }
    let a = input.canonicalize().map_err(|e| format!("Cannot resolve input path: {e}"))?;
    let b = output.canonicalize().map_err(|e| format!("Cannot resolve output path: {e}"))?;
    Ok(a == b)
}

fn write_new_file_atomically(output: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temp = NamedTempFile::new_in(parent).map_err(|e| format!("Cannot create output temporary file: {e}"))?;
    temp.write_all(bytes).map_err(|e| format!("Cannot write compressed file: {e}"))?;
    temp.as_file().sync_all().map_err(|e| format!("Cannot flush compressed file: {e}"))?;
    temp.persist_noclobber(output)
        .map_err(|e| format!("Cannot save output without overwriting an existing file: {}", e.error))?;
    Ok(())
}

fn copy_new_file_atomically(source: &Path, output: &Path) -> Result<(), String> {
    let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temp = NamedTempFile::new_in(parent).map_err(|e| format!("Cannot create output temporary file: {e}"))?;
    let mut src = File::open(source).map_err(|e| format!("Cannot open source file: {e}"))?;
    std::io::copy(&mut src, &mut temp).map_err(|e| format!("Cannot copy file: {e}"))?;
    temp.as_file().sync_all().map_err(|e| format!("Cannot flush file: {e}"))?;
    temp.persist_noclobber(output)
        .map_err(|e| format!("Cannot save without overwriting an existing file: {}", e.error))?;
    Ok(())
}

fn reduction(original: u64, result: u64) -> f64 {
    if original == 0 {
        0.0
    } else {
        (original.saturating_sub(result) as f64 / original as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT_100KB: u64 = 100_000;

    fn noisy_image(w: u32, h: u32) -> DynamicImage {
        let mut state: u32 = 12345;
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let n = (state >> 24) as u8;
            image::Rgb([((x * 255 / w) as u8) ^ (n >> 3), ((y * 255 / h) as u8) ^ (n >> 3), n])
        }))
    }

    fn request(input: &Path, output: &Path, target: u64, format: &str, mode: &str, resize: bool) -> CompressionRequest {
        CompressionRequest {
            input_path: input.to_string_lossy().into_owned(),
            output_path: output.to_string_lossy().into_owned(),
            target_bytes: target,
            output_format: format.into(),
            mode: mode.into(),
            allow_resize: resize,
            preserve_transparency: true,
            min_quality: 20,
        }
    }

    #[test]
    fn reduction_math() {
        assert_eq!(reduction(100, 100), 0.0);
        assert_eq!(reduction(0, 0), 0.0);
        assert!((reduction(200, 100) - 50.0).abs() < 0.001);
    }

    #[test]
    fn auto_format_choice() {
        assert_eq!(choose_image_format("auto", true, true).unwrap(), "png");
        assert_eq!(choose_image_format("auto", true, false).unwrap(), "jpeg");
        assert_eq!(choose_image_format("auto", false, true).unwrap(), "jpeg");
    }

    #[test]
    fn large_photo_to_100kb_maximum_is_verified_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        noisy_image(1600, 1200).save(&input).unwrap();
        assert!(fs::metadata(&input).unwrap().len() > 1_000_000, "test input should be large");
        let output = dir.path().join("photo_compressed.jpg");
        let result = compress(request(&input, &output, LIMIT_100KB, "jpeg", "maximum", true)).unwrap();
        let on_disk = fs::metadata(&output).unwrap().len();
        assert!(on_disk <= LIMIT_100KB, "saved file is {on_disk} bytes");
        assert_eq!(on_disk, result.output_bytes);
        assert!(result.met_target);
        // The saved file must be a readable JPEG.
        assert!(image::open(&output).is_ok());
    }

    #[test]
    fn closest_mode_never_exceeds_limit_and_is_not_worse_than_maximum() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        noisy_image(1600, 1200).save(&input).unwrap();
        let out_max = dir.path().join("max.jpg");
        let out_closest = dir.path().join("closest.jpg");
        compress(request(&input, &out_max, LIMIT_100KB, "jpeg", "maximum", true)).unwrap();
        compress(request(&input, &out_closest, LIMIT_100KB, "jpeg", "closest", true)).unwrap();
        let max_len = fs::metadata(&out_max).unwrap().len();
        let closest_len = fs::metadata(&out_closest).unwrap().len();
        assert!(closest_len <= LIMIT_100KB);
        assert!(closest_len >= max_len);
    }

    #[test]
    fn photo_to_50kb_is_verified_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        noisy_image(1200, 900).save(&input).unwrap();
        let output = dir.path().join("out.jpg");
        compress(request(&input, &output, 50_000, "jpeg", "maximum", true)).unwrap();
        assert!(fs::metadata(&output).unwrap().len() <= 50_000);
    }

    #[test]
    fn impossible_target_fails_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        noisy_image(800, 600).save(&input).unwrap();
        let output = dir.path().join("out.jpg");
        let result = compress(request(&input, &output, MIN_TARGET_BYTES, "jpeg", "maximum", false));
        assert!(result.is_err());
        assert!(!output.exists(), "no oversized file may be left behind");
    }

    #[test]
    fn small_source_in_auto_mode_is_copied_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("small.png");
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(64, 64, image::Rgb([10, 120, 200])))
            .save(&input)
            .unwrap();
        let output = dir.path().join("small_compressed.png");
        let result = compress(request(&input, &output, LIMIT_100KB, "auto", "maximum", true)).unwrap();
        assert_eq!(fs::read(&input).unwrap(), fs::read(&output).unwrap());
        assert_eq!(result.reduction_percent, 0.0);
    }

    #[test]
    fn transparent_png_keeps_alpha() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("logo.png");
        let rgba = image::RgbaImage::from_fn(200, 200, |x, y| image::Rgba([x as u8, y as u8, 90, (x as u8).saturating_add(20)]));
        DynamicImage::ImageRgba8(rgba).save(&input).unwrap();
        let output = dir.path().join("logo_compressed.png");
        compress(request(&input, &output, 1_000_000, "png", "maximum", true)).unwrap();
        assert!(image::open(&output).unwrap().color().has_alpha());
    }

    #[test]
    fn existing_output_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        noisy_image(300, 300).save(&input).unwrap();
        let output = dir.path().join("exists.jpg");
        fs::write(&output, b"keep me").unwrap();
        assert!(compress(request(&input, &output, LIMIT_100KB, "jpeg", "maximum", true)).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"keep me");
    }

    #[test]
    fn original_file_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("photo.png");
        noisy_image(500, 500).save(&input).unwrap();
        let before = fs::read(&input).unwrap();
        let output = dir.path().join("out.jpg");
        compress(request(&input, &output, 30_000, "jpeg", "maximum", true)).unwrap();
        assert_eq!(before, fs::read(&input).unwrap());
    }

    #[test]
    fn misleading_extension_is_rejected_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("fake.jpg");
        fs::write(&input, b"this is not an image").unwrap();
        let output = dir.path().join("out.jpg");
        assert!(compress(request(&input, &output, LIMIT_100KB, "jpeg", "maximum", true)).is_err());
        assert!(!output.exists());
    }
}
