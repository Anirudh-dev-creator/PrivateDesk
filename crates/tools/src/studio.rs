//! Image Studio: edit one image and save the result as a new file.

use crate::common::*;
use image::{DynamicImage, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

fn default_format() -> String {
    "jpeg".into()
}
fn default_quality() -> u8 {
    90
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditParams {
    pub input_path: String,
    #[serde(default)]
    pub crop: Option<CropRect>,
    #[serde(default)]
    pub resize_width: Option<u32>,
    #[serde(default)]
    pub resize_height: Option<u32>,
    #[serde(default)]
    pub rotate: i32,
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub flip_vertical: bool,
    #[serde(default)]
    pub brightness: i32,
    #[serde(default)]
    pub contrast: f32,
    #[serde(default)]
    pub grayscale: bool,
    #[serde(default)]
    pub sepia: bool,
    #[serde(default)]
    pub blur: f32,
    #[serde(default)]
    pub sharpen: bool,
    #[serde(default = "default_format")]
    pub output_format: String,
    #[serde(default = "default_quality")]
    pub jpeg_quality: u8,
    #[serde(default)]
    pub dpi: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub format: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditPreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct ExifEntry {
    pub tag: String,
    pub value: String,
}

pub fn image_info(path: &str) -> Result<ImageInfo, String> {
    let image = load_image(path)?;
    let p = Path::new(path);
    let bytes = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?.len();
    let format = ImageReader::open(p)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.format())
        .map(|f| format!("{f:?}"))
        .unwrap_or_else(|| "Unknown".into());
    Ok(ImageInfo {
        path: path.to_string(),
        name: p.file_name().and_then(|s| s.to_str()).unwrap_or("image").to_string(),
        bytes,
        width: image.width(),
        height: image.height(),
        has_alpha: image.color().has_alpha(),
        format,
    })
}

pub fn read_exif(path: &str) -> Result<Vec<ExifEntry>, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open file: {e}"))?;
    let mut reader = BufReader::new(&file);
    match exif::Reader::new().read_from_container(&mut reader) {
        Ok(data) => Ok(data
            .fields()
            .map(|f| ExifEntry {
                tag: f.tag.to_string(),
                value: f.display_value().with_unit(&data).to_string(),
            })
            .collect()),
        Err(_) => Ok(Vec::new()),
    }
}

fn sepia(image: &DynamicImage) -> DynamicImage {
    let mut rgba = image.to_rgba8();
    for p in rgba.pixels_mut() {
        let [r, g, b, a] = p.0;
        let (rf, gf, bf) = (f32::from(r), f32::from(g), f32::from(b));
        let nr = (0.393 * rf + 0.769 * gf + 0.189 * bf).min(255.0);
        let ng = (0.349 * rf + 0.686 * gf + 0.168 * bf).min(255.0);
        let nb = (0.272 * rf + 0.534 * gf + 0.131 * bf).min(255.0);
        p.0 = [nr as u8, ng as u8, nb as u8, a];
    }
    DynamicImage::ImageRgba8(rgba)
}

pub fn apply_edits(mut img: DynamicImage, p: &EditParams) -> Result<DynamicImage, String> {
    if let Some(c) = &p.crop {
        if c.width == 0 || c.height == 0 || c.x.saturating_add(c.width) > img.width() || c.y.saturating_add(c.height) > img.height() {
            return Err(format!(
                "The crop area must fit inside the image ({} × {} pixels).",
                img.width(),
                img.height()
            ));
        }
        let rgba = img.to_rgba8();
        img = DynamicImage::ImageRgba8(image::imageops::crop_imm(&rgba, c.x, c.y, c.width, c.height).to_image());
    }
    img = match p.rotate.rem_euclid(360) {
        0 => img,
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => return Err("Rotation must be 0, 90, 180 or 270 degrees.".into()),
    };
    if p.flip_horizontal {
        img = img.fliph();
    }
    if p.flip_vertical {
        img = img.flipv();
    }
    let (w, h) = (img.width(), img.height());
    let target = match (p.resize_width, p.resize_height) {
        (Some(nw), Some(nh)) => Some((nw, nh)),
        (Some(nw), None) => Some((nw, ((u64::from(h) * u64::from(nw)) as f64 / f64::from(w)).round() as u32)),
        (None, Some(nh)) => Some((((u64::from(w) * u64::from(nh)) as f64 / f64::from(h)).round() as u32, nh)),
        (None, None) => None,
    };
    if let Some((nw, nh)) = target {
        if nw == 0 || nh == 0 || u64::from(nw) * u64::from(nh) > MAX_PIXELS {
            return Err("The new size must be at least 1 pixel and at most 40 megapixels.".into());
        }
        img = img.resize_exact(nw, nh, image::imageops::FilterType::Lanczos3);
    }
    if p.brightness != 0 {
        img = img.brighten(p.brightness.clamp(-100, 100));
    }
    if p.contrast != 0.0 {
        img = img.adjust_contrast(p.contrast.clamp(-100.0, 100.0));
    }
    if p.grayscale {
        img = img.grayscale();
    }
    if p.sepia {
        img = sepia(&img);
    }
    if p.blur > 0.0 {
        img = img.blur(p.blur.min(20.0));
    }
    if p.sharpen {
        img = img.unsharpen(1.5, 5);
    }
    Ok(img)
}

/// Returns (encoded bytes, file extension).
pub fn encode_output(img: &DynamicImage, p: &EditParams) -> Result<(Vec<u8>, &'static str), String> {
    match p.output_format.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => {
            let rgb = flatten_on_color(img, [255, 255, 255]);
            Ok((encode_jpeg(&rgb, p.jpeg_quality, p.dpi)?, "jpg"))
        }
        "png" => Ok((encode_with_format(img, ImageFormat::Png)?, "png")),
        "webp" => Ok((encode_with_format(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::WebP)?, "webp")),
        "bmp" => Ok((encode_with_format(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::Bmp)?, "bmp")),
        "tiff" | "tif" => Ok((encode_with_format(&DynamicImage::ImageRgba8(img.to_rgba8()), ImageFormat::Tiff)?, "tiff")),
        _ => Err("Unknown output format.".into()),
    }
}

pub fn preview(p: &EditParams) -> Result<EditPreview, String> {
    let edited = apply_edits(load_image(&p.input_path)?, p)?;
    let (bytes, _) = encode_output(&edited, p)?;
    Ok(EditPreview {
        data_url: thumbnail_data_url(&edited)?,
        width: edited.width(),
        height: edited.height(),
        bytes: bytes.len() as u64,
        format: p.output_format.clone(),
    })
}

pub fn save(p: &EditParams, output_path: &str) -> Result<SavedFile, String> {
    let out = Path::new(output_path);
    if out.exists() {
        return Err("That file already exists. Choose a new name; originals are never overwritten.".into());
    }
    let edited = apply_edits(load_image(&p.input_path)?, p)?;
    let (bytes, _) = encode_output(&edited, p)?;
    write_new_file(out, &bytes)?;
    let on_disk = verify_saved(out, bytes.len())?;
    Ok(SavedFile {
        output_path: output_path.to_string(),
        bytes: on_disk,
        width: edited.width(),
        height: edited.height(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_png(dir: &Path, w: u32, h: u32) -> String {
        let path = dir.join("src.png");
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])))
            .save(&path)
            .unwrap();
        path.to_string_lossy().into_owned()
    }

    fn params(input: &str) -> EditParams {
        EditParams {
            input_path: input.into(),
            crop: None,
            resize_width: None,
            resize_height: None,
            rotate: 0,
            flip_horizontal: false,
            flip_vertical: false,
            brightness: 0,
            contrast: 0.0,
            grayscale: false,
            sepia: false,
            blur: 0.0,
            sharpen: false,
            output_format: "jpeg".into(),
            jpeg_quality: 90,
            dpi: None,
        }
    }

    #[test]
    fn crop_rotate_resize_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let src = make_png(dir.path(), 400, 200);
        let mut p = params(&src);
        p.crop = Some(CropRect { x: 0, y: 0, width: 300, height: 100 });
        p.rotate = 90;
        let out = apply_edits(load_image(&src).unwrap(), &p).unwrap();
        assert_eq!((out.width(), out.height()), (100, 300));
        p.crop = None;
        p.rotate = 0;
        p.resize_width = Some(200);
        let out = apply_edits(load_image(&src).unwrap(), &p).unwrap();
        assert_eq!((out.width(), out.height()), (200, 100));
    }

    #[test]
    fn crop_outside_image_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let src = make_png(dir.path(), 100, 100);
        let mut p = params(&src);
        p.crop = Some(CropRect { x: 50, y: 50, width: 100, height: 100 });
        assert!(apply_edits(load_image(&src).unwrap(), &p).is_err());
    }

    #[test]
    fn save_all_formats_verified_on_disk_and_original_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let src = make_png(dir.path(), 120, 90);
        let before = fs::read(&src).unwrap();
        for (fmt, name) in [("jpeg", "a.jpg"), ("png", "a.png"), ("webp", "a.webp"), ("bmp", "a.bmp"), ("tiff", "a.tiff")] {
            let mut p = params(&src);
            p.output_format = fmt.into();
            p.sepia = true;
            p.brightness = 10;
            p.sharpen = true;
            let out = dir.path().join(name);
            let saved = save(&p, out.to_str().unwrap()).unwrap();
            assert_eq!(saved.bytes, fs::metadata(&out).unwrap().len(), "{fmt}");
            assert!(image::open(&out).is_ok(), "{fmt} output must be readable");
        }
        assert_eq!(before, fs::read(&src).unwrap());
    }

    #[test]
    fn existing_output_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let src = make_png(dir.path(), 50, 50);
        let out = dir.path().join("exists.jpg");
        fs::write(&out, b"keep").unwrap();
        assert!(save(&params(&src), out.to_str().unwrap()).is_err());
        assert_eq!(fs::read(&out).unwrap(), b"keep");
    }

    #[test]
    fn exif_missing_returns_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let src = make_png(dir.path(), 20, 20);
        assert!(read_exif(&src).unwrap().is_empty());
    }
}
