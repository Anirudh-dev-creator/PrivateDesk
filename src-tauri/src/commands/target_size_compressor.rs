//! Thin Tauri wrappers around the `pd_compressor` engine crate.

use pd_compressor::{CompressionRequest, CompressionResult, CompressorFileInfo};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub ghostscript: bool,
}

#[tauri::command]
pub async fn inspect_compressor_file(input_path: String) -> Result<CompressorFileInfo, String> {
    tauri::async_runtime::spawn_blocking(move || pd_compressor::inspect_file(&input_path))
        .await
        .map_err(|e| format!("File inspection failed: {e}"))?
}

#[tauri::command]
pub async fn compress_to_target(request: CompressionRequest) -> Result<CompressionResult, String> {
    pd_compressor::reset_cancel();
    tauri::async_runtime::spawn_blocking(move || pd_compressor::compress(request))
        .await
        .map_err(|e| format!("Compression task failed: {e}"))?
}

#[tauri::command]
pub async fn compress_into_folder(request: pd_compressor::FolderRequest) -> Result<CompressionResult, String> {
    pd_compressor::reset_cancel();
    tauri::async_runtime::spawn_blocking(move || pd_compressor::compress_into_folder(request))
        .await
        .map_err(|e| format!("Compression task failed: {e}"))?
}

#[tauri::command]
pub fn cancel_compression() {
    pd_compressor::request_cancel();
}

#[tauri::command]
pub fn engine_status() -> EngineStatus {
    EngineStatus {
        ghostscript: pd_compressor::ghostscript_available(),
    }
}

#[tauri::command]
pub fn reveal_in_folder(path: String) -> Result<(), String> {
    let file = std::path::PathBuf::from(&path);
    if !file.is_file() {
        return Err("The file could not be found.".into());
    }
    if !cfg!(windows) {
        return Err("Showing a file in its folder is only available on Windows.".into());
    }
    // Arguments are passed individually; no shell is involved.
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", file.display()))
        .spawn()
        .map_err(|e| format!("Could not open File Explorer: {e}"))?;
    Ok(())
}
