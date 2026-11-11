//! File and text utilities: hashes, QR codes, Base64, duplicates, bulk rename.

use crate::common::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

// ---------- hashes ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HashInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub md5: String,
}

pub fn hash_file(path: &str) -> Result<HashInfo, String> {
    let p = Path::new(path);
    let meta = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?;
    if !meta.is_file() {
        return Err("Please choose a file, not a folder.".into());
    }
    let mut file = File::open(p).map_err(|e| format!("Cannot open file: {e}"))?;
    let mut sha = Sha256::new();
    let mut md = Md5::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("Read error: {e}"))?;
        if n == 0 {
            break;
        }
        sha.update(&buf[..n]);
        md.update(&buf[..n]);
    }
    Ok(HashInfo {
        path: path.to_string(),
        name: p.file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string(),
        size_bytes: meta.len(),
        sha256: format!("{:x}", sha.finalize()),
        md5: format!("{:x}", md.finalize()),
    })
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mut sha = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("Read error: {e}"))?;
        if n == 0 {
            break;
        }
        sha.update(&buf[..n]);
    }
    Ok(format!("{:x}", sha.finalize()))
}

// ---------- QR code ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrPreview {
    pub data_url: String,
    pub width: u32,
}

fn qr_image(text: &str, module_px: u32) -> Result<RgbImage, String> {
    if text.is_empty() {
        return Err("Enter some text or a link first.".into());
    }
    if text.len() > 2000 {
        return Err("The text is too long for a QR code (limit 2,000 characters).".into());
    }
    let module_px = module_px.clamp(2, 40);
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|e| format!("Cannot make a QR code: {e}"))?;
    let width = code.width();
    let colors = code.to_colors();
    let border = 4_usize;
    let total = ((width + 2 * border) as u32) * module_px;
    let mut img = RgbImage::from_pixel(total, total, Rgb([255, 255, 255]));
    for y in 0..width {
        for x in 0..width {
            if colors[y * width + x] == qrcode::Color::Dark {
                let px0 = ((x + border) as u32) * module_px;
                let py0 = ((y + border) as u32) * module_px;
                for dy in 0..module_px {
                    for dx in 0..module_px {
                        img.put_pixel(px0 + dx, py0 + dy, Rgb([0, 0, 0]));
                    }
                }
            }
        }
    }
    Ok(img)
}

pub fn qr_preview(text: &str, module_px: u32) -> Result<QrPreview, String> {
    let img = qr_image(text, module_px)?;
    let width = img.width();
    let bytes = encode_with_format(&DynamicImage::ImageRgb8(img), ImageFormat::Png)?;
    Ok(QrPreview { data_url: data_url("image/png", &bytes), width })
}

pub fn qr_save(text: &str, module_px: u32, output_path: &str) -> Result<SavedFile, String> {
    let out = Path::new(output_path);
    if out.exists() {
        return Err("That file already exists. Choose a new name.".into());
    }
    let img = qr_image(text, module_px)?;
    let (w, h) = img.dimensions();
    let bytes = encode_with_format(&DynamicImage::ImageRgb8(img), ImageFormat::Png)?;
    write_new_file(out, &bytes)?;
    let on_disk = verify_saved(out, bytes.len())?;
    Ok(SavedFile { output_path: output_path.into(), bytes: on_disk, width: w, height: h })
}

// ---------- Base64 ----------

pub fn b64_encode(text: &str) -> String {
    STANDARD.encode(text.as_bytes())
}

pub fn b64_decode(text: &str) -> Result<String, String> {
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = STANDARD.decode(cleaned).map_err(|_| "That is not valid Base64.".to_string())?;
    String::from_utf8(bytes).map_err(|_| "The decoded data is not readable text.".to_string())
}

// ---------- save text ----------

pub fn save_text_file(path: &str, content: &str) -> Result<(), String> {
    write_new_file(Path::new(path), content.as_bytes())
}

// ---------- duplicate finder ----------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub size_bytes: u64,
    pub sha256: String,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub groups: Vec<DuplicateGroup>,
    pub files_scanned: u64,
    pub wasted_bytes: u64,
    pub truncated: bool,
}

const MAX_SCAN_FILES: u64 = 100_000;

/// Read-only: reports duplicates, never deletes or moves anything.
pub fn find_duplicates(folder: &str, min_size: u64) -> Result<DuplicateReport, String> {
    let root = PathBuf::from(folder);
    if !root.is_dir() {
        return Err("Please choose a folder.".into());
    }
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut stack = vec![root];
    let mut scanned: u64 = 0;
    let mut truncated = false;
    'walk: while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                let Ok(meta) = entry.metadata() else { continue };
                scanned += 1;
                if scanned > MAX_SCAN_FILES {
                    truncated = true;
                    break 'walk;
                }
                if meta.len() >= min_size.max(1) {
                    by_size.entry(meta.len()).or_default().push(path);
                }
            }
        }
    }
    let mut groups = Vec::new();
    let mut wasted = 0_u64;
    for (size, files) in by_size.into_iter().filter(|(_, f)| f.len() > 1) {
        let mut by_hash: HashMap<String, Vec<String>> = HashMap::new();
        for f in files {
            if let Ok(h) = sha256_file(&f) {
                by_hash.entry(h).or_default().push(f.to_string_lossy().into_owned());
            }
        }
        for (hash, mut list) in by_hash.into_iter().filter(|(_, l)| l.len() > 1) {
            list.sort();
            wasted += size * (list.len() as u64 - 1);
            groups.push(DuplicateGroup { size_bytes: size, sha256: hash, files: list });
        }
    }
    groups.sort_by(|a, b| (b.size_bytes * b.files.len() as u64).cmp(&(a.size_bytes * a.files.len() as u64)));
    Ok(DuplicateReport { groups, files_scanned: scanned.min(MAX_SCAN_FILES), wasted_bytes: wasted, truncated })
}

// ---------- bulk rename ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub paths: Vec<String>,
    /// File name without extension. Tokens: {name} = old name, {n} = counter.
    pub pattern: String,
    pub start: u32,
    pub pad: u32,
    pub find: String,
    pub replace: String,
    pub apply: bool,
}

#[derive(Debug, Serialize)]
pub struct RenameItem {
    pub from: String,
    pub to: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RenameReport {
    pub items: Vec<RenameItem>,
    pub applied: bool,
}

fn bad_char(c: char) -> bool {
    c.is_control() || "<>:\"/\\|?*".contains(c)
}

fn reserved(stem: &str) -> bool {
    let up = stem.to_ascii_uppercase();
    matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (up.len() == 4 && (up.starts_with("COM") || up.starts_with("LPT")) && up.as_bytes()[3].is_ascii_digit())
}

pub fn bulk_rename(req: &RenameRequest) -> Result<RenameReport, String> {
    if req.paths.is_empty() {
        return Err("Choose at least one file.".into());
    }
    if req.paths.len() > 5000 {
        return Err("Please rename at most 5,000 files at a time.".into());
    }
    if req.paths.len() > 1 && !req.pattern.contains("{name}") && !req.pattern.contains("{n}") {
        return Err("With several files the pattern must contain {name} or {n}, otherwise the new names would clash.".into());
    }
    let mut items = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (i, from) in req.paths.iter().enumerate() {
        let path = Path::new(from);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();
        let base = if req.find.is_empty() { stem } else { stem.replace(&req.find, &req.replace) };
        let number = format!("{:0width$}", u64::from(req.start) + i as u64, width = req.pad.min(12) as usize);
        let new_stem = req.pattern.replace("{name}", &base).replace("{n}", &number);
        let target = path.parent().unwrap_or(Path::new(".")).join(format!("{new_stem}{ext}"));

        let mut ok = true;
        let mut message = String::new();
        if !path.is_file() {
            ok = false;
            message = "File not found.".into();
        } else if new_stem.trim().is_empty() || new_stem.chars().any(bad_char) || new_stem.ends_with('.') || new_stem.ends_with(' ') {
            ok = false;
            message = "The new name is empty or contains characters Windows does not allow.".into();
        } else if reserved(&new_stem) {
            ok = false;
            message = "That name is reserved by Windows.".into();
        } else if target != path && target.exists() {
            ok = false;
            message = "A file with that name already exists.".into();
        } else if !seen.insert(target.to_string_lossy().to_lowercase()) {
            ok = false;
            message = "Two files would get the same name.".into();
        }
        items.push(RenameItem { from: from.clone(), to: target.to_string_lossy().into_owned(), ok, message });
    }

    let mut applied = false;
    if req.apply {
        if items.iter().all(|it| it.ok) {
            for it in items.iter_mut() {
                if it.from == it.to {
                    it.message = "Unchanged".into();
                } else if let Err(e) = fs::rename(&it.from, &it.to) {
                    it.ok = false;
                    it.message = format!("Could not rename: {e}");
                } else {
                    it.message = "Renamed".into();
                }
            }
            applied = true;
        }
    }
    Ok(RenameReport { items, applied })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("abc.txt");
        fs::write(&f, b"abc").unwrap();
        let h = hash_file(f.to_str().unwrap()).unwrap();
        assert_eq!(h.sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(h.md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(h.size_bytes, 3);
    }

    #[test]
    fn qr_is_a_valid_png_and_saves_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("qr.png");
        let saved = qr_save("https://example.com", 8, out.to_str().unwrap()).unwrap();
        assert_eq!(saved.bytes, fs::metadata(&out).unwrap().len());
        assert!(image::open(&out).is_ok());
        assert!(qr_preview("", 8).is_err());
    }

    #[test]
    fn base64_round_trip_with_unicode() {
        let text = "नमस्ते PrivateDesk";
        assert_eq!(b64_decode(&b64_encode(text)).unwrap(), text);
        assert!(b64_decode("***").is_err());
    }

    #[test]
    fn duplicates_are_found_and_nothing_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"same content").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("b.txt"), b"same content").unwrap();
        fs::write(dir.path().join("c.txt"), b"different!!!").unwrap();
        let report = find_duplicates(dir.path().to_str().unwrap(), 1).unwrap();
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 2);
        assert_eq!(report.wasted_bytes, 12);
        assert!(dir.path().join("a.txt").exists() && dir.path().join("c.txt").exists());
    }

    fn rename_request(paths: Vec<String>, pattern: &str, apply: bool) -> RenameRequest {
        RenameRequest { paths, pattern: pattern.into(), start: 1, pad: 2, find: String::new(), replace: String::new(), apply }
    }

    #[test]
    fn bulk_rename_numbers_files_and_keeps_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("x.txt");
        let b = dir.path().join("y.txt");
        fs::write(&a, b"1").unwrap();
        fs::write(&b, b"2").unwrap();
        let paths = vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned()];
        let preview = bulk_rename(&rename_request(paths.clone(), "doc_{n}", false)).unwrap();
        assert!(preview.items.iter().all(|i| i.ok) && !preview.applied);
        assert!(a.exists());
        let done = bulk_rename(&rename_request(paths, "doc_{n}", true)).unwrap();
        assert!(done.applied);
        assert!(dir.path().join("doc_01.txt").exists() && dir.path().join("doc_02.txt").exists());
        assert!(!a.exists());
    }

    #[test]
    fn bulk_rename_refuses_clashes_and_bad_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("x.txt");
        let b = dir.path().join("y.txt");
        fs::write(&a, b"1").unwrap();
        fs::write(&b, b"2").unwrap();
        let paths = vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned()];
        assert!(bulk_rename(&rename_request(paths.clone(), "same", true)).is_err());
        let report = bulk_rename(&rename_request(paths, "bad?{n}", true)).unwrap();
        assert!(!report.applied);
        assert!(a.exists() && b.exists());
    }
}
