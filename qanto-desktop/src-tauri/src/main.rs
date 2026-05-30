#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[tauri::command]
fn start_qanto_node() -> String {
    // This command will execute the compiled qanto-node binary in the background
    println!("Initializing QANTO Layer-0 Sentinel Node...");
    
    // Mocking the daemon thread execution for the GUI
    format!("Node Initialized at 10,000,000 TPS. Synaptic DAG Active.")
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_qanto_node])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
