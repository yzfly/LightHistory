use crate::db::segment;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
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

pub struct MsgRow {
    pub id: String,
    pub sender: String,
    pub text: String,
    pub created_at: String,
}

/// 写入一个会话（uuid 去重：updated_at 未变则跳过，变了则整体替换消息）。
#[allow(clippy::too_many_arguments)]
fn upsert_conversation(
    conn: &Connection,
    result: &mut ImportResult,
    id: &str,
    source: &str,
    title: &str,
    project: &str,
    account: &str,
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
         (id, source, title, project, account, created_at, updated_at, message_count, user_chars, assistant_chars)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![id, source, title, project, account, created_at, updated_at,
                msgs.len() as i64, user_chars as i64, assistant_chars as i64],
    )?;

    conn.execute("DELETE FROM messages_fts WHERE conv_id = ?1", params![id])?;
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

// ---------- claude.ai 导出 zip ----------

/// 导入 claude.ai 官方导出 zip（conversations.json + users.json 账号信息）。
pub fn import_claude_zip(conn: &mut Connection, zip_path: &str) -> Result<ImportResult, String> {
    let file = File::open(zip_path).map_err(|e| format!("无法打开文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("不是有效的 zip: {e}"))?;

    // users.json：账号 uuid → 展示名（邮箱优先）
    let mut accounts: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(mut f) = archive.by_name("users.json") {
        let mut raw = String::new();
        if f.read_to_string(&mut raw).is_ok() {
            if let Ok(users) = serde_json::from_str::<Vec<Value>>(&raw) {
                for u in &users {
                    let uuid = u["uuid"].as_str().unwrap_or_default().to_string();
                    let email = u["email_address"].as_str().unwrap_or_default();
                    let name = u["full_name"].as_str().unwrap_or_default();
                    let display = if !email.is_empty() { email } else { name };
                    if !uuid.is_empty() && !display.is_empty() {
                        accounts.insert(uuid, display.to_string());
                    }
                }
            }
        }
    }
    let default_account = accounts.values().next().cloned().unwrap_or_default();

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
        let account_uuid = c["account"]["uuid"].as_str().unwrap_or_default();
        let account = accounts
            .get(account_uuid)
            .cloned()
            .unwrap_or_else(|| default_account.clone());

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
            &account,
            c["created_at"].as_str().unwrap_or_default(),
            c["updated_at"].as_str().unwrap_or_default(),
            &msgs,
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

// ---------- Claude Code ----------

/// 读本机 Claude Code 登录账号（尽力而为，读不到则标记为「本机」）。
fn claude_code_account() -> String {
    let path = match dirs::home_dir() {
        Some(h) => h.join(".claude.json"),
        None => return "本机".into(),
    };
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<Value>(&raw) {
            for key in ["oauthAccount", "account"] {
                let email = v[key]["emailAddress"]
                    .as_str()
                    .or_else(|| v[key]["email"].as_str());
                if let Some(e) = email {
                    if !e.is_empty() {
                        return e.to_string();
                    }
                }
            }
        }
    }
    "本机".into()
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
    let account = claude_code_account();

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
            if let Err(e) = import_claude_code_session(&tx, &mut result, &path, &project, &account)
            {
                eprintln!("跳过 {}: {e}", path.display());
            } else if let Some(stem) = path.file_stem() {
                imported_ids.push(stem.to_string_lossy().to_string());
            }
        }
        // 正文 jsonl 已被 Claude Code 清理的会话，从 sessions-index.json 抢救元数据
        if let Err(e) = import_index_remnants(&tx, &mut result, &pd.path(), &imported_ids, &account)
        {
            eprintln!("索引解析失败 {}: {e}", project);
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

fn import_index_remnants(
    conn: &Connection,
    result: &mut ImportResult,
    project_dir: &std::path::Path,
    imported_ids: &[String],
    account: &str,
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
            continue;
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
            conn, result, &sid, "claude_code", &title, project, account, created, modified, &msgs,
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
    account: &str,
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

    let display_project = project.replacen('-', "/", 1).replace('-', "/");

    upsert_conversation(
        conn,
        result,
        &session_id,
        "claude_code",
        &title,
        &display_project,
        account,
        &first_ts,
        &last_ts,
        &msgs,
    )
    .map_err(|e| e.to_string())
}

// ---------- 通用数据文件导入（JSON / CSV）----------

/// 导入数据文件：
/// - LightHistory 备份 JSON（{"lighthistory": 1, "conversations": [...]}) → 原样恢复
/// - 通用消息数组 JSON（[{contact/sender/text/time...}]）→ 按联系人归组
/// - 通用消息 CSV（带表头）→ 同上
pub fn import_data_file(conn: &mut Connection, path: &str) -> Result<ImportResult, String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".csv") {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("无法读取文件: {e}"))?;
        let records = parse_csv_records(&raw)?;
        return import_generic_records(conn, records);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| format!("无法读取文件: {e}"))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| format!("JSON 解析失败: {e}"))?;
    if v.get("lighthistory").is_some() && v.get("conversations").is_some() {
        return restore_backup(conn, &v);
    }
    if let Value::Array(items) = v {
        let records = items.iter().filter_map(json_record_to_map).collect::<Vec<_>>();
        return import_generic_records(conn, records);
    }
    Err("无法识别的文件格式：支持 LightHistory 备份 JSON、消息数组 JSON、CSV".into())
}

type Record = BTreeMap<String, String>;

fn json_record_to_map(v: &Value) -> Option<Record> {
    let obj = v.as_object()?;
    let mut m = Record::new();
    for (k, val) in obj {
        let s = match val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        m.insert(k.to_lowercase(), s);
    }
    Some(m)
}

/// 极简 CSV 解析（支持双引号转义）。
fn parse_csv_records(raw: &str) -> Result<Vec<Record>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut field = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut in_quotes = false;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    if row.iter().any(|f| !f.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        if row.iter().any(|f| !f.trim().is_empty()) {
            rows.push(row);
        }
    }
    if rows.len() < 2 {
        return Err("CSV 内容不足（需要表头 + 至少一行数据）".into());
    }
    let header: Vec<String> = rows[0].iter().map(|h| h.trim().to_lowercase()).collect();
    Ok(rows[1..]
        .iter()
        .map(|r| header.iter().cloned().zip(r.iter().cloned()).collect::<Record>())
        .collect())
}

fn pick<'a>(rec: &'a Record, keys: &[&str]) -> Option<&'a String> {
    keys.iter()
        .find_map(|k| rec.get(*k))
        .filter(|s| !s.trim().is_empty())
}

/// 时间归一化：unix 秒/毫秒或字符串。
fn norm_time(s: &str) -> String {
    let t = s.trim();
    if let Ok(n) = t.parse::<i64>() {
        let secs = if n > 100_000_000_000 { n / 1000 } else { n };
        if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
            return dt.to_rfc3339();
        }
    }
    t.to_string()
}

fn import_generic_records(
    conn: &mut Connection,
    records: Vec<Record>,
) -> Result<ImportResult, String> {
    const CONTACT_KEYS: &[&str] = &[
        "contact", "chat", "room", "group", "talker", "conversation", "会话", "联系人", "群聊",
    ];
    const SENDER_KEYS: &[&str] =
        &["sender", "from", "name", "nickname", "talker_name", "发送者", "昵称"];
    const TEXT_KEYS: &[&str] = &["content", "text", "msg", "message", "body", "内容", "消息"];
    const TIME_KEYS: &[&str] = &[
        "createtime", "create_time", "time", "timestamp", "date", "datetime", "时间",
    ];
    const SELF_KEYS: &[&str] = &["issend", "is_send", "is_self", "self", "issender", "是否发送"];

    let mut groups: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    for rec in &records {
        let text = match pick(rec, TEXT_KEYS) {
            Some(t) => t.clone(),
            None => continue,
        };
        let contact = pick(rec, CONTACT_KEYS)
            .cloned()
            .unwrap_or_else(|| "未命名会话".into());
        let time = pick(rec, TIME_KEYS).map(|t| norm_time(t)).unwrap_or_default();
        let is_self = pick(rec, SELF_KEYS)
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "是"))
            .unwrap_or(false);
        let sender = if is_self {
            "human".to_string()
        } else {
            pick(rec, SENDER_KEYS).cloned().unwrap_or_else(|| contact.clone())
        };
        groups.entry(contact).or_default().push((sender, text, time));
    }
    if groups.is_empty() {
        return Err("没有解析到任何消息：请确认文件包含 内容/content 字段".into());
    }

    let mut result = ImportResult::default();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (contact, mut msgs) in groups {
        msgs.sort_by(|a, b| a.2.cmp(&b.2));
        let created = msgs.first().map(|m| m.2.clone()).unwrap_or_default();
        let updated = msgs.last().map(|m| m.2.clone()).unwrap_or_default();
        let conv_id = format!("im-{:x}", fnv1a(&contact));
        let rows: Vec<MsgRow> = msgs
            .into_iter()
            .enumerate()
            .map(|(i, (sender, text, time))| MsgRow {
                id: format!("{conv_id}-{i}"),
                sender,
                text,
                created_at: time,
            })
            .collect();
        upsert_conversation(
            &tx, &mut result, &conv_id, "im_chat", &contact, "", "", &created, &updated, &rows,
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

/// FNV-1a 稳定散列：联系人名 → 会话 id。
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ---------- 备份恢复 ----------

fn restore_backup(conn: &mut Connection, v: &Value) -> Result<ImportResult, String> {
    let convs = v["conversations"]
        .as_array()
        .ok_or("备份文件缺少 conversations 数组")?;
    let mut result = ImportResult::default();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for c in convs {
        let id = c["id"].as_str().unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let msgs: Vec<MsgRow> = c["messages"]
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|m| MsgRow {
                        id: m["id"].as_str().unwrap_or_default().to_string(),
                        sender: m["sender"].as_str().unwrap_or("human").to_string(),
                        text: m["text"].as_str().unwrap_or_default().to_string(),
                        created_at: m["created_at"].as_str().unwrap_or_default().to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        upsert_conversation(
            &tx,
            &mut result,
            id,
            c["source"].as_str().unwrap_or("claude_web"),
            c["title"].as_str().unwrap_or_default(),
            c["project"].as_str().unwrap_or_default(),
            c["account"].as_str().unwrap_or_default(),
            c["created_at"].as_str().unwrap_or_default(),
            c["updated_at"].as_str().unwrap_or_default(),
            &msgs,
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(result)
}
