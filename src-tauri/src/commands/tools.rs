//! Thin Tauri wrappers around the `pd_tools` crate. Heavy work runs off the UI thread.

use pd_tools::common::SavedFile;
use pd_tools::passport::{PhotoParams, PhotoPreview, SheetParams};
use pd_tools::studio::{EditParams, EditPreview, ExifEntry, ImageInfo};
use pd_tools::utils::{DuplicateReport, HashInfo, QrPreview, RenameReport, RenameRequest};

async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("Background task failed: {e}"))?
}

// ----- Image Studio -----

#[tauri::command]
pub async fn image_info(path: String) -> Result<ImageInfo, String> {
    blocking(move || pd_tools::studio::image_info(&path)).await
}

#[tauri::command]
pub async fn studio_preview(params: EditParams) -> Result<EditPreview, String> {
    blocking(move || pd_tools::studio::preview(&params)).await
}

#[tauri::command]
pub async fn studio_save(params: EditParams, output_path: String) -> Result<SavedFile, String> {
    blocking(move || pd_tools::studio::save(&params, &output_path)).await
}

#[tauri::command]
pub async fn read_exif(path: String) -> Result<Vec<ExifEntry>, String> {
    blocking(move || pd_tools::studio::read_exif(&path)).await
}

// ----- Passport studio -----

#[tauri::command]
pub async fn passport_preview(params: PhotoParams) -> Result<PhotoPreview, String> {
    blocking(move || pd_tools::passport::photo_preview(&params)).await
}

#[tauri::command]
pub async fn passport_export(params: PhotoParams, output_path: String, format: String) -> Result<SavedFile, String> {
    blocking(move || pd_tools::passport::export_photo(&params, &output_path, &format)).await
}

#[tauri::command]
pub async fn sheet_preview(params: SheetParams) -> Result<PhotoPreview, String> {
    blocking(move || pd_tools::passport::sheet_preview(&params)).await
}

#[tauri::command]
pub async fn sheet_export(params: SheetParams, output_path: String, format: String) -> Result<SavedFile, String> {
    blocking(move || pd_tools::passport::export_sheet(&params, &output_path, &format)).await
}

// ----- Utilities -----

#[tauri::command]
pub async fn hash_file(path: String) -> Result<HashInfo, String> {
    blocking(move || pd_tools::utils::hash_file(&path)).await
}

#[tauri::command]
pub async fn qr_preview(text: String, module_px: u32) -> Result<QrPreview, String> {
    blocking(move || pd_tools::utils::qr_preview(&text, module_px)).await
}

#[tauri::command]
pub async fn qr_save(text: String, module_px: u32, output_path: String) -> Result<SavedFile, String> {
    blocking(move || pd_tools::utils::qr_save(&text, module_px, &output_path)).await
}

#[tauri::command]
pub fn b64_encode(text: String) -> String {
    pd_tools::utils::b64_encode(&text)
}

#[tauri::command]
pub fn b64_decode(text: String) -> Result<String, String> {
    pd_tools::utils::b64_decode(&text)
}

#[tauri::command]
pub async fn find_duplicates(folder: String, min_size: u64) -> Result<DuplicateReport, String> {
    blocking(move || pd_tools::utils::find_duplicates(&folder, min_size)).await
}

#[tauri::command]
pub async fn bulk_rename(request: RenameRequest) -> Result<RenameReport, String> {
    blocking(move || pd_tools::utils::bulk_rename(&request)).await
}

#[tauri::command]
pub async fn save_text_file(path: String, content: String) -> Result<(), String> {
    blocking(move || pd_tools::utils::save_text_file(&path, &content)).await
}
