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

    pub fn import_file(conn: &mut Connection, path: &str) -> Result<importer::ImportResult, String> {
        importer::import_data_file(conn, path)
    }

    pub fn list_accounts(conn: &Connection) -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare("SELECT DISTINCT account FROM conversations WHERE account != ''")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn export_library(conn: &Connection, dest: &str) -> Result<usize, String> {
        let mut stmt = conn
            .prepare("SELECT id, source, title, project, account, created_at, updated_at FROM conversations")
            .map_err(|e| e.to_string())?;
        let metas: Vec<(String, String, String, String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut msg_stmt = conn
            .prepare("SELECT id, sender, text, created_at FROM messages WHERE conv_id = ?1 ORDER BY idx")
            .map_err(|e| e.to_string())?;
        let mut convs = Vec::new();
        for (id, source, title, project, account, created_at, updated_at) in &metas {
            let messages = msg_stmt
                .query_map([id], |r| {
                    Ok(serde_json::json!({
                        "id": r.get::<_, String>(0)?,
                        "sender": r.get::<_, String>(1)?,
                        "text": r.get::<_, String>(2)?,
                        "created_at": r.get::<_, String>(3)?,
                    }))
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            convs.push(serde_json::json!({
                "id": id, "source": source, "title": title, "project": project,
                "account": account, "created_at": created_at, "updated_at": updated_at,
                "messages": messages,
            }));
        }
        let dump = serde_json::json!({"lighthistory": 1, "conversations": convs});
        std::fs::write(dest, serde_json::to_string(&dump).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(metas.len())
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
            commands::import_data_file,
            commands::list_accounts,
            commands::export_library,
            commands::backup_db,
            commands::export_conversation,
            commands::export_batch,
            commands::get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
