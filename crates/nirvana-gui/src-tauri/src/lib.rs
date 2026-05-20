#[tauri::command]
fn get_app_info(app: tauri::AppHandle) -> serde_json::Value {
    let info = app.package_info();

    serde_json::json!({
        "name": info.name,
        "version": info.version.to_string(),
        "authors": info.authors,
        "description": info.description,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
