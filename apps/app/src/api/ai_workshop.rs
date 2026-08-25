use crate::ai_workshop;

// === AI-WORKSHOP START ===
pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
	ai_workshop::init()
}
// === AI-WORKSHOP END ===
