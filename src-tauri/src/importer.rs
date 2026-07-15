use crate::db::segment;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

#[derive(Serialize, Default)]
pub struct ImportResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub messages: usize,
}

struct MsgRow {
    id: String,
    sender: String,
    text: String,
    created_at: String,
}

/// 写入一个会话（uuid 去重：updated_at 未变则跳过，变了则整体替换消息）。
fn upsert_conversation(
    conn: &Connection,
    result: &mut ImportResult,
    id: &str,
    source: &str,
    title: &str,
    project: &str,
    created_at: &str,
    updated_at: &str,
    msgs: &[MsgRow],
) -> rusqlite::Result<()> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT updated_at FROM conversations WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map(Some)
        .unwrap_or(None);

    match &existing {
        Some(prev) if prev == updated_at => {
            result.skipped += 1;
            return Ok(());
        }
        Some(_) => result.updated += 1,
        None => result.imported += 1,
    }

    let user_chars: usize = msgs
        .iter()
        .filter(|m| m.sender == "human")
        .map(|m| m.text.chars().count())
        .sum();
    let assistant_chars: usize = msgs
        .iter()
        .filter(|m| m.sender != "human")
        .map(|m| m.text.chars().count())
        .sum();

    conn.execute(
        "INSERT OR REPLACE INTO conversations
         (id, source, title, project, created_at, updated_at, message_count, user_chars, assistant_chars)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![id, source, title, project, created_at, updated_at,
                msgs.len() as i64, user_chars as i64, assistant_chars as i64],
    )?;

    conn.execute(
        "DELETE FROM messages_fts WHERE conv_id = ?1",
        params![id],
    )?;
    conn.execute("DELETE FROM messages WHERE conv_id = ?1", params![id])?;

    let mut msg_stmt = conn.prepare_cached(
        "INSERT INTO messages (id, conv_id, idx, sender, text, created_at) VALUES (?1,?2,?3,?4,?5,?6)",
    )?;
    let mut fts_stmt = conn.prepare_cached(
        "INSERT INTO messages_fts (seg_text, conv_id, msg_id) VALUES (?1,?2,?3)",
    )?;
    for (i, m) in msgs.iter().enumerate() {
        msg_stmt.execute(params![m.id, id, i as i64, m.sender, m.text, m.created_at])?;
        // 标题也拼进第一条消息的索引文本，让标题可搜
        let seg = if i == 0 {
            segment(&format!("{} {}", title, m.text))
        } else {
            segment(&m.text)
        };
        fts_stmt.execute(params![seg, id, m.id])?;
        result.messages += 1;
    }
    Ok(())
}

fn text_of_content_blocks(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                    b.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// 导入 claude.ai 官方导出 zip（conversations.json）。
pub fn import_claude_zip(conn: &mut Connection, zip_path: &str) -> Result<ImportResult, String> {
    let file = File::open(zip_path).map_err(|e| format!("无法打开文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("不是有效的 zip: {e}"))?;

    let mut raw = String::new();
    archive
        .by_name("conversations.json")
        .map_err(|_| "zip 中没有 conversations.json，请确认是 claude.ai 的数据导出包".to_string())?
        .read_to_string(&mut raw)
        .map_err(|e| format!("读取失败: {e}"))?;

    let convs: Vec<Value> =
        serde_json::from_str(&raw).map_err(|e| format!("conversations.json 解析失败: {e}"))?;

    let mut result = ImportResult::default();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for c in &convs {
        let id = c["uuid"].as_str().unwrap_or_default().to_string();
        if id.is_empty() {
            continue;
        }
        let mut msgs: Vec<MsgRow> = Vec::new();
        if let Some(list) = c["chat_messages"].as_array() {
            for m in list {
                let mut text = m["text"].as_str().unwrap_or_default().to_string();
                if text.is_empty() {
                    text = text_of_content_blocks(&m["content"]);
                }
                if text.trim().is_empty() {
                    continue;
                }
                msgs.push(MsgRow {
                    id: m["uuid"].as_str().unwrap_or_default().to_string(),
                    sender: if m["sender"].as_str() == Some("human") {
                        "human".into()
                    } else {
                        "assistant".into()
                    },
                    text,
                    created_at: m["created_at"].as_str().unwrap_or_default().to_string(),
                });
            }
        }
        let name = c["name"].as_str().unwrap_or_default();
        let title = if name.trim().is_empty() {
            msgs.first()
                .map(|m| m.text.chars().take(40).collect::<String>())
                .unwrap_or_else(|| "（空会话）".into())
        } else {
            name.to_string()
        };
        upsert_conversation(
            &tx,
            &mut result,
            &id,
            "claude_web",
            &title,
            "",
            c["created_at"].as_str().unwrap_or_default(),
            c["updated_at"].as_str().unwrap_or_default(),
            &msgs,
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

/// 扫描 ~/.claude/projects/ 导入本机 Claude Code 会话。
pub fn import_claude_code(conn: &mut Connection) -> Result<ImportResult, String> {
    let root: PathBuf = dirs::home_dir()
        .ok_or("找不到用户目录")?
        .join(".claude")
        .join("projects");
    if !root.is_dir() {
        return Err("本机没有找到 ~/.claude/projects 目录".into());
    }

    let mut result = ImportResult::default();
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let project_dirs = std::fs::read_dir(&root).map_err(|e| e.to_string())?;
    for pd in project_dirs.flatten() {
        if !pd.path().is_dir() {
            continue;
        }
        let project = pd.file_name().to_string_lossy().to_string();
        let files = match std::fs::read_dir(pd.path()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut imported_ids: Vec<String> = Vec::new();
        for f in files.flatten() {
            let path = f.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Err(e) = import_claude_code_session(&tx, &mut result, &path, &project) {
                eprintln!("跳过 {}: {e}", path.display());
            } else if let Some(stem) = path.file_stem() {
                imported_ids.push(stem.to_string_lossy().to_string());
            }
        }
        // 正文 jsonl 已被 Claude Code 清理的会话，从 sessions-index.json 抢救元数据
        if let Err(e) = import_index_remnants(&tx, &mut result, &pd.path(), &imported_ids) {
            eprintln!("索引解析失败 {}: {e}", project);
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

/// sessions-index.json 里有、但正文 jsonl 已被清理的会话：
/// 保留摘要 + 首条提问，避免历史彻底丢失。
fn import_index_remnants(
    conn: &Connection,
    result: &mut ImportResult,
    project_dir: &std::path::Path,
    imported_ids: &[String],
) -> Result<(), String> {
    let index_path = project_dir.join("sessions-index.json");
    if !index_path.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&index_path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let entries = match v["entries"].as_array() {
        Some(e) => e,
        None => return Ok(()),
    };
    for e in entries {
        let sid = e["sessionId"].as_str().unwrap_or_default().to_string();
        if sid.is_empty() || imported_ids.contains(&sid) {
            continue;
        }
        if e["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        let full_path = e["fullPath"].as_str().unwrap_or_default();
        if !full_path.is_empty() && std::path::Path::new(full_path).is_file() {
            continue; // 正文还在，别的轮次会导入
        }
        let first_prompt = e["firstPrompt"].as_str().unwrap_or_default().trim().to_string();
        if first_prompt.is_empty() || first_prompt.starts_with('<') {
            continue;
        }
        let summary = e["summary"].as_str().unwrap_or_default();
        let title = if summary.is_empty() {
            first_prompt.chars().take(40).collect()
        } else {
            summary.to_string()
        };
        let created = e["created"].as_str().unwrap_or_default();
        let modified = e["modified"].as_str().unwrap_or_default();
        let project = e["projectPath"].as_str().unwrap_or_default();
        let msgs = vec![MsgRow {
            id: format!("{sid}-first"),
            sender: "human".into(),
            text: first_prompt,
            created_at: created.to_string(),
        }];
        upsert_conversation(
            conn, result, &sid, "claude_code", &title, project, created, modified, &msgs,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn import_claude_code_session(
    conn: &Connection,
    result: &mut ImportResult,
    path: &std::path::Path,
    project: &str,
) -> Result<(), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let session_id = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut msgs: Vec<MsgRow> = Vec::new();
    let mut summary_title = String::new();
    let mut first_ts = String::new();
    let mut last_ts = String::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let typ = v["type"].as_str().unwrap_or_default();
        if typ == "summary" {
            if let Some(s) = v["summary"].as_str() {
                summary_title = s.to_string();
            }
            continue;
        }
        if typ != "user" && typ != "assistant" {
            continue;
        }
        if v["isSidechain"].as_bool() == Some(true) {
            continue;
        }
        let text = text_of_content_blocks(&v["message"]["content"]);
        let text = text.trim();
        if text.is_empty()
            || text.starts_with("<command-")
            || text.starts_with("<local-command-")
            || text.starts_with("<system-reminder>")
        {
            continue;
        }
        let ts = v["timestamp"].as_str().unwrap_or_default().to_string();
        if first_ts.is_empty() {
            first_ts = ts.clone();
        }
        if !ts.is_empty() {
            last_ts = ts.clone();
        }
        msgs.push(MsgRow {
            id: v["uuid"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}-{}", session_id, msgs.len())),
            sender: if typ == "user" { "human".into() } else { "assistant".into() },
            text: text.to_string(),
            created_at: ts,
        });
    }

    if msgs.is_empty() {
        return Ok(());
    }

    let title = if !summary_title.is_empty() {
        summary_title
    } else {
        msgs.iter()
            .find(|m| m.sender == "human")
            .map(|m| m.text.chars().take(40).collect())
            .unwrap_or_else(|| "（无标题会话）".into())
    };

    // 项目目录名转成可读路径（best-effort，仅用于展示）
    let display_project = project.replacen('-', "/", 1).replace('-', "/");

    upsert_conversation(
        conn,
        result,
        &session_id,
        "claude_code",
        &title,
        &display_project,
        &first_ts,
        &last_ts,
        &msgs,
    )
    .map_err(|e| e.to_string())
}
