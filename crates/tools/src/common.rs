use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::codecs::jpeg::{JpegEncoder, PixelDensity};
use image::{DynamicImage, ExtendedColorType, ImageEncoder, ImageFormat, ImageReader, RgbImage};
use serde::Serialize;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use tempfile::NamedTempFile;

pub const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 40_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFile {
    pub output_path: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
}

/// Writes a brand-new file. Fails (never overwrites) if the path already exists.
pub fn write_new_file(output: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err("The output folder does not exist.".into());
    }
    let mut temp = NamedTempFile::new_in(parent).map_err(|e| format!("Cannot create temporary file: {e}"))?;
    temp.write_all(bytes).map_err(|e| format!("Cannot write file: {e}"))?;
    temp.as_file().sync_all().map_err(|e| format!("Cannot flush file: {e}"))?;
    temp.persist_noclobber(output)
        .map_err(|e| format!("Cannot save without overwriting an existing file: {}", e.error))?;
    Ok(())
}

/// Confirms the file on disk has the expected size; removes it if not.
pub fn verify_saved(output: &Path, expected: usize) -> Result<u64, String> {
    let len = fs::metadata(output)
        .map_err(|e| format!("Cannot verify saved file: {e}"))?
        .len();
    if len != expected as u64 {
        let _ = fs::remove_file(output);
        return Err("The saved file did not match what was written. It was removed.".into());
    }
    Ok(len)
}

pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

pub fn parse_hex_color(text: &str) -> Result<[u8; 3], String> {
    let t = text.trim().trim_start_matches('#');
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Colour must look like #FFFFFF.".into());
    }
    let v = u32::from_str_radix(t, 16).map_err(|_| "Invalid colour.".to_string())?;
    Ok([((v >> 16) & 255) as u8, ((v >> 8) & 255) as u8, (v & 255) as u8])
}

pub fn flatten_on_color(image: &DynamicImage, bg: [u8; 3]) -> RgbImage {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    RgbImage::from_fn(w, h, |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = u16::from(p[3]);
        let blend = |c: u8, b: u8| -> u8 { ((u16::from(c) * a + u16::from(b) * (255 - a) + 127) / 255) as u8 };
        image::Rgb([blend(p[0], bg[0]), blend(p[1], bg[1]), blend(p[2], bg[2])])
    })
}

pub fn encode_jpeg(rgb: &RgbImage, quality: u8, dpi: Option<u32>) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100));
    if let Some(d) = dpi {
        encoder.set_pixel_density(PixelDensity::dpi(d.clamp(1, 65535) as u16));
    }
    encoder
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG encoding failed: {e}"))?;
    Ok(buf)
}

pub fn encode_with_format(image: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut buf), format)
        .map_err(|e| format!("Encoding failed: {e}"))?;
    Ok(buf)
}

/// Small preview image as a data URL (PNG if transparent, otherwise JPEG).
pub fn thumbnail_data_url(image: &DynamicImage) -> Result<String, String> {
    let thumb = if image.width().max(image.height()) > 800 {
        image.thumbnail(800, 800)
    } else {
        image.clone()
    };
    if thumb.color().has_alpha() {
        Ok(data_url("image/png", &encode_with_format(&thumb, ImageFormat::Png)?))
    } else {
        let rgb = flatten_on_color(&thumb, [255, 255, 255]);
        Ok(data_url("image/jpeg", &encode_jpeg(&rgb, 85, None)?))
    }
}

pub fn load_image(path: &str) -> Result<DynamicImage, String> {
    let p = Path::new(path);
    let meta = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?;
    if !meta.is_file() {
        return Err("Please choose a file, not a folder.".into());
    }
    if meta.len() > MAX_INPUT_BYTES {
        return Err("The file is larger than the 512 MB safety limit.".into());
    }
    let (w, h) = image::image_dimensions(p).map_err(|e| format!("This does not look like a supported image: {e}"))?;
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err("The image is larger than 40 megapixels, which is too big for safe processing.".into());
    }
    ImageReader::open(p)
        .map_err(|e| format!("Cannot open image: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("Cannot identify image format: {e}"))?
        .decode()
        .map_err(|e| format!("Image decode failed: {e}"))
}
