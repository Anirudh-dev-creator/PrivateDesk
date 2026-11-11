//! Passport / ID photo preparation and print sheets.
//! Positioning is manual (zoom and offsets). There is no face detection or
//! background removal in this version.

use crate::common::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoParams {
    pub input_path: String,
    pub width_mm: f32,
    pub height_mm: f32,
    pub dpi: u32,
    pub zoom: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub background: String,
    #[serde(default)]
    pub brightness: i32,
    #[serde(default)]
    pub contrast: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetParams {
    pub photo: PhotoParams,
    pub paper_width_mm: f32,
    pub paper_height_mm: f32,
    pub margin_mm: f32,
    pub spacing_mm: f32,
    /// 0 = as many as fit.
    pub count: u32,
    pub cut_guides: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoPreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub count: u32,
}

pub fn mm_to_px(mm: f32, dpi: u32) -> u32 {
    ((mm / 25.4) * dpi as f32).round().max(1.0) as u32
}

fn validate(p: &PhotoParams) -> Result<(), String> {
    if !(10.0..=300.0).contains(&p.width_mm) || !(10.0..=300.0).contains(&p.height_mm) {
        return Err("Photo width and height must be between 10 and 300 mm.".into());
    }
    if !(100..=600).contains(&p.dpi) {
        return Err("Resolution must be between 100 and 600 DPI.".into());
    }
    Ok(())
}

pub fn render_photo(p: &PhotoParams) -> Result<RgbImage, String> {
    validate(p)?;
    let bg = parse_hex_color(&p.background)?;
    let mut img = load_image(&p.input_path)?;
    if p.brightness != 0 {
        img = img.brighten(p.brightness.clamp(-100, 100));
    }
    if p.contrast != 0.0 {
        img = img.adjust_contrast(p.contrast.clamp(-100.0, 100.0));
    }
    let (sw, sh) = (img.width() as f32, img.height() as f32);
    let aspect = p.width_mm / p.height_mm;
    let (bw, bh) = if sw / sh > aspect { (sh * aspect, sh) } else { (sw, sw / aspect) };
    let zoom = p.zoom.clamp(1.0, 4.0);
    let (cw, ch) = (bw / zoom, bh / zoom);
    let ox = p.offset_x.clamp(-1.0, 1.0);
    let oy = p.offset_y.clamp(-1.0, 1.0);
    let cx = sw / 2.0 + ox * (sw - cw) / 2.0;
    let cy = sh / 2.0 + oy * (sh - ch) / 2.0;
    let x0 = (cx - cw / 2.0).clamp(0.0, (sw - cw).max(0.0));
    let y0 = (cy - ch / 2.0).clamp(0.0, (sh - ch).max(0.0));
    let rgba = img.to_rgba8();
    let cropped = image::imageops::crop_imm(
        &rgba,
        x0.round() as u32,
        y0.round() as u32,
        (cw.round() as u32).max(1),
        (ch.round() as u32).max(1),
    )
    .to_image();
    let out_w = mm_to_px(p.width_mm, p.dpi);
    let out_h = mm_to_px(p.height_mm, p.dpi);
    let resized = image::imageops::resize(&cropped, out_w, out_h, image::imageops::FilterType::Lanczos3);
    Ok(flatten_on_color(&DynamicImage::ImageRgba8(resized), bg))
}

fn outline(img: &mut RgbImage, x: i64, y: i64, w: u32, h: u32, color: Rgb<u8>) {
    let (iw, ih) = (i64::from(img.width()), i64::from(img.height()));
    let (x0, y0, x1, y1) = (x - 1, y - 1, x + i64::from(w), y + i64::from(h));
    for xx in x0..=x1 {
        for yy in [y0, y1] {
            if xx >= 0 && yy >= 0 && xx < iw && yy < ih {
                img.put_pixel(xx as u32, yy as u32, color);
            }
        }
    }
    for yy in y0..=y1 {
        for xx in [x0, x1] {
            if xx >= 0 && yy >= 0 && xx < iw && yy < ih {
                img.put_pixel(xx as u32, yy as u32, color);
            }
        }
    }
}

/// Returns the finished sheet and the number of photos placed.
pub fn render_sheet(s: &SheetParams) -> Result<(RgbImage, u32), String> {
    validate(&s.photo)?;
    if !(20.0..=1000.0).contains(&s.paper_width_mm) || !(20.0..=1000.0).contains(&s.paper_height_mm) {
        return Err("Paper size must be between 20 and 1000 mm.".into());
    }
    if s.margin_mm < 0.0 || s.spacing_mm < 0.0 || s.margin_mm > 100.0 || s.spacing_mm > 50.0 {
        return Err("Margin and spacing must be between 0 and the allowed maximum.".into());
    }
    let dpi = s.photo.dpi;
    let photo = render_photo(&s.photo)?;
    let (pw, ph) = photo.dimensions();
    let paper_w = mm_to_px(s.paper_width_mm, dpi);
    let paper_h = mm_to_px(s.paper_height_mm, dpi);
    if u64::from(paper_w) * u64::from(paper_h) > 80_000_000 {
        return Err("The sheet would be too large at this resolution.".into());
    }
    let margin = (s.margin_mm / 25.4 * dpi as f32).round() as i64;
    let gap = (s.spacing_mm / 25.4 * dpi as f32).round() as i64;
    let usable_w = i64::from(paper_w) - 2 * margin;
    let usable_h = i64::from(paper_h) - 2 * margin;
    let (pw_i, ph_i) = (i64::from(pw), i64::from(ph));
    if usable_w < pw_i || usable_h < ph_i {
        return Err("The photo does not fit on this paper with these margins.".into());
    }
    let cols = ((usable_w + gap) / (pw_i + gap)).max(1);
    let rows = ((usable_h + gap) / (ph_i + gap)).max(1);
    let capacity = (cols * rows) as u32;
    let count = if s.count == 0 { capacity } else { s.count.min(capacity) };
    let cols_used = i64::from(count).min(cols);
    let rows_used = (i64::from(count) + cols - 1) / cols;
    let block_w = cols_used * pw_i + (cols_used - 1) * gap;
    let start_x = (i64::from(paper_w) - block_w) / 2;
    let start_y = margin;
    let _ = rows_used;

    let mut sheet = RgbImage::from_pixel(paper_w, paper_h, Rgb([255, 255, 255]));
    let positions: Vec<(i64, i64)> = (0..i64::from(count))
        .map(|i| (start_x + (i % cols) * (pw_i + gap), start_y + (i / cols) * (ph_i + gap)))
        .collect();
    if s.cut_guides {
        for (x, y) in &positions {
            outline(&mut sheet, *x, *y, pw, ph, Rgb([150, 150, 150]));
        }
    }
    for (x, y) in &positions {
        image::imageops::replace(&mut sheet, &photo, *x, *y);
    }
    Ok((sheet, count))
}

pub fn photo_preview(p: &PhotoParams) -> Result<PhotoPreview, String> {
    let photo = render_photo(p)?;
    let (w, h) = photo.dimensions();
    Ok(PhotoPreview {
        data_url: thumbnail_data_url(&DynamicImage::ImageRgb8(photo))?,
        width: w,
        height: h,
        count: 1,
    })
}

pub fn sheet_preview(s: &SheetParams) -> Result<PhotoPreview, String> {
    let (sheet, count) = render_sheet(s)?;
    let (w, h) = sheet.dimensions();
    Ok(PhotoPreview {
        data_url: thumbnail_data_url(&DynamicImage::ImageRgb8(sheet))?,
        width: w,
        height: h,
        count,
    })
}

fn encode_rgb(img: &RgbImage, format: &str, dpi: u32) -> Result<Vec<u8>, String> {
    match format.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => encode_jpeg(img, 95, Some(dpi)),
        "png" => encode_with_format(&DynamicImage::ImageRgb8(img.clone()), ImageFormat::Png),
        _ => Err("Choose JPG or PNG.".into()),
    }
}

pub fn export_photo(p: &PhotoParams, output_path: &str, format: &str) -> Result<SavedFile, String> {
    let out = Path::new(output_path);
    if out.exists() {
        return Err("That file already exists. Choose a new name.".into());
    }
    let photo = render_photo(p)?;
    let bytes = encode_rgb(&photo, format, p.dpi)?;
    write_new_file(out, &bytes)?;
    let on_disk = verify_saved(out, bytes.len())?;
    Ok(SavedFile { output_path: output_path.into(), bytes: on_disk, width: photo.width(), height: photo.height() })
}

pub fn export_sheet(s: &SheetParams, output_path: &str, format: &str) -> Result<SavedFile, String> {
    let out = Path::new(output_path);
    if out.exists() {
        return Err("That file already exists. Choose a new name.".into());
    }
    let (sheet, _) = render_sheet(s)?;
    let bytes = encode_rgb(&sheet, format, s.photo.dpi)?;
    write_new_file(out, &bytes)?;
    let on_disk = verify_saved(out, bytes.len())?;
    Ok(SavedFile { output_path: output_path.into(), bytes: on_disk, width: sheet.width(), height: sheet.height() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn source(dir: &Path) -> String {
        let path = dir.join("portrait.png");
        DynamicImage::ImageRgb8(RgbImage::from_fn(1200, 1600, |x, y| Rgb([(x % 250) as u8, (y % 250) as u8, 90])))
            .save(&path)
            .unwrap();
        path.to_string_lossy().into_owned()
    }

    fn photo(input: &str) -> PhotoParams {
        PhotoParams {
            input_path: input.into(),
            width_mm: 35.0,
            height_mm: 45.0,
            dpi: 300,
            zoom: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            background: "#FFFFFF".into(),
            brightness: 0,
            contrast: 0.0,
        }
    }

    #[test]
    fn photo_has_exact_pixel_size_for_mm_and_dpi() {
        let dir = tempfile::tempdir().unwrap();
        let src = source(dir.path());
        let img = render_photo(&photo(&src)).unwrap();
        assert_eq!(img.width(), mm_to_px(35.0, 300));
        assert_eq!(img.height(), mm_to_px(45.0, 300));
    }

    #[test]
    fn zoom_and_offsets_do_not_panic_at_extremes() {
        let dir = tempfile::tempdir().unwrap();
        let src = source(dir.path());
        for (z, ox, oy) in [(1.0, -1.0, -1.0), (4.0, 1.0, 1.0), (2.5, 0.3, -0.7)] {
            let mut p = photo(&src);
            p.zoom = z;
            p.offset_x = ox;
            p.offset_y = oy;
            assert!(render_photo(&p).is_ok());
        }
    }

    #[test]
    fn a4_sheet_has_paper_size_and_fits_several_photos() {
        let dir = tempfile::tempdir().unwrap();
        let src = source(dir.path());
        let s = SheetParams {
            photo: photo(&src),
            paper_width_mm: 210.0,
            paper_height_mm: 297.0,
            margin_mm: 5.0,
            spacing_mm: 2.0,
            count: 0,
            cut_guides: true,
        };
        let (sheet, count) = render_sheet(&s).unwrap();
        assert_eq!(sheet.width(), mm_to_px(210.0, 300));
        assert_eq!(sheet.height(), mm_to_px(297.0, 300));
        assert!(count >= 20, "expected many photos on A4, got {count}");
    }

    #[test]
    fn sheet_export_is_verified_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let src = source(dir.path());
        let s = SheetParams {
            photo: photo(&src),
            paper_width_mm: 101.6,
            paper_height_mm: 152.4,
            margin_mm: 3.0,
            spacing_mm: 2.0,
            count: 6,
            cut_guides: false,
        };
        let out = dir.path().join("sheet.jpg");
        let saved = export_sheet(&s, out.to_str().unwrap(), "jpeg").unwrap();
        assert_eq!(saved.bytes, fs::metadata(&out).unwrap().len());
    }

    #[test]
    fn photo_larger_than_paper_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let src = source(dir.path());
        let s = SheetParams {
            photo: photo(&src),
            paper_width_mm: 30.0,
            paper_height_mm: 30.0,
            margin_mm: 1.0,
            spacing_mm: 1.0,
            count: 0,
            cut_guides: false,
        };
        assert!(render_sheet(&s).is_err());
    }

    #[test]
    fn bad_colour_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let src = source(dir.path());
        let mut p = photo(&src);
        p.background = "blue".into();
        assert!(render_photo(&p).is_err());
    }
}
