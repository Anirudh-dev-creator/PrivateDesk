mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::target_size_compressor::inspect_compressor_file,
            commands::target_size_compressor::compress_to_target,
            commands::target_size_compressor::cancel_compression,
            commands::target_size_compressor::reveal_in_folder,
            commands::target_size_compressor::engine_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PrivateDesk");
}
