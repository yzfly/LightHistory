mod commands;
mod db;
mod importer;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppDb(pub Mutex<Connection>);

/// 供集成测试复用内部逻辑（不经过 Tauri State）。
pub mod testing {
    use super::*;
    use std::path::Path;

    pub fn open_db(path: &Path) -> rusqlite::Result<Connection> {
        db::open(path)
    }

    pub fn import_zip(conn: &mut Connection, path: &str) -> Result<importer::ImportResult, String> {
        importer::import_claude_zip(conn, path)
    }

    pub fn import_code(conn: &mut Connection) -> Result<importer::ImportResult, String> {
        importer::import_claude_code(conn)
    }

    pub fn search(conn: &Connection, query: &str) -> Result<Vec<(String, String)>, String> {
        let fts = db::build_fts_query(query);
        let mut stmt = conn
            .prepare(
                "SELECT c.title, snippet(messages_fts, 0, '[[', ']]', '…', 24)
                 FROM messages_fts f JOIN conversations c ON c.id = f.conv_id
                 WHERE messages_fts MATCH ?1 ORDER BY rank LIMIT 100",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([fts], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn quick_stats(conn: &Connection) -> Result<(i64, i64, i64), String> {
        conn.query_row(
            "SELECT (SELECT COUNT(*) FROM conversations),
                    (SELECT COUNT(*) FROM messages),
                    (SELECT COALESCE(SUM(user_chars),0) FROM conversations)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| e.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = db::open(&data_dir.join("lighthistory.db"))?;
            app.manage(AppDb(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_conversations,
            commands::get_conversation,
            commands::search,
            commands::import_claude_zip,
            commands::import_claude_code,
            commands::export_conversation,
            commands::export_batch,
            commands::get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
