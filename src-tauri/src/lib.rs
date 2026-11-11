mod commands;

use commands::{target_size_compressor as tsc, tools};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            tsc::inspect_compressor_file,
            tsc::compress_to_target,
            tsc::compress_into_folder,
            tsc::cancel_compression,
            tsc::reveal_in_folder,
            tsc::engine_status,
            tools::image_info,
            tools::studio_preview,
            tools::studio_save,
            tools::read_exif,
            tools::passport_preview,
            tools::passport_export,
            tools::sheet_preview,
            tools::sheet_export,
            tools::hash_file,
            tools::qr_preview,
            tools::qr_save,
            tools::b64_encode,
            tools::b64_decode,
            tools::find_duplicates,
            tools::bulk_rename,
            tools::save_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PrivateDesk");
}
