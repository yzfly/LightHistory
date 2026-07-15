use crate::db::build_fts_query;
use crate::importer::{self, ImportResult};
use crate::AppDb;
use rusqlite::params;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct ConvMeta {
    pub id: String,
    pub source: String,
    pub title: String,
    pub project: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub user_chars: i64,
    pub assistant_chars: i64,
}

#[derive(Serialize)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct ConvDetail {
    pub meta: ConvMeta,
    pub messages: Vec<Message>,
}

#[derive(Serialize)]
pub struct SearchHit {
    pub conv_id: String,
    pub title: String,
    pub source: String,
    pub snippet: String,
    pub msg_id: String,
    pub updated_at: String,
}

fn row_to_meta(r: &rusqlite::Row) -> rusqlite::Result<ConvMeta> {
    Ok(ConvMeta {
        id: r.get(0)?,
        source: r.get(1)?,
        title: r.get(2)?,
        project: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        message_count: r.get(6)?,
        user_chars: r.get(7)?,
        assistant_chars: r.get(8)?,
    })
}

const META_COLS: &str =
    "id, source, title, project, created_at, updated_at, message_count, user_chars, assistant_chars";

#[tauri::command]
pub fn list_conversations(
    db: State<AppDb>,
    source: Option<String>,
    sort: Option<String>,
) -> Result<Vec<ConvMeta>, String> {
    let conn = db.0.lock().unwrap();
    let order = match sort.as_deref() {
        Some("created") => "created_at DESC",
        Some("messages") => "message_count DESC",
        _ => "updated_at DESC",
    };
    let (sql, has_source) = match &source {
        Some(s) if !s.is_empty() => (
            format!("SELECT {META_COLS} FROM conversations WHERE source = ?1 ORDER BY {order}"),
            true,
        ),
        _ => (
            format!("SELECT {META_COLS} FROM conversations ORDER BY {order}"),
            false,
        ),
    };
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = if has_source {
        stmt.query_map(params![source.unwrap()], row_to_meta)
    } else {
        stmt.query_map([], row_to_meta)
    }
    .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_conversation(db: State<AppDb>, id: String) -> Result<ConvDetail, String> {
    get_conversation_inner(&db, &id)
}

#[tauri::command]
pub fn search(db: State<AppDb>, query: String) -> Result<Vec<SearchHit>, String> {
    let fts = build_fts_query(&query);
    if fts.is_empty() {
        return Ok(vec![]);
    }
    let conn = db.0.lock().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT f.conv_id, c.title, c.source, c.updated_at, f.msg_id,
                    snippet(messages_fts, 0, '[[', ']]', '…', 24)
             FROM messages_fts f
             JOIN conversations c ON c.id = f.conv_id
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT 100",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![fts], |r| {
            let raw: String = r.get(5)?;
            Ok(SearchHit {
                conv_id: r.get(0)?,
                title: r.get(1)?,
                source: r.get(2)?,
                updated_at: r.get(3)?,
                msg_id: r.get(4)?,
                // 去掉分词时插入的空格，恢复中文可读性
                snippet: crate::db::unsegment_snippet(&raw),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_claude_zip(db: State<AppDb>, path: String) -> Result<ImportResult, String> {
    let mut conn = db.0.lock().unwrap();
    importer::import_claude_zip(&mut conn, &path)
}

#[tauri::command]
pub fn import_claude_code(db: State<AppDb>) -> Result<ImportResult, String> {
    let mut conn = db.0.lock().unwrap();
    importer::import_claude_code(&mut conn)
}

// ---------- 导出 ----------

fn sender_label(sender: &str) -> &'static str {
    if sender == "human" { "用户" } else { "Claude" }
}

fn render_markdown(d: &ConvDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", d.meta.title));
    out.push_str(&format!(
        "> 来源: {} · 消息数: {} · 时间: {} — {}\n\n---\n\n",
        d.meta.source, d.meta.message_count, d.meta.created_at, d.meta.updated_at
    ));
    for m in &d.messages {
        out.push_str(&format!("## {} · {}\n\n{}\n\n", sender_label(&m.sender), m.created_at, m.text));
    }
    out
}

fn render_txt(d: &ConvDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n{}\n\n", d.meta.title, "=".repeat(40)));
    for m in &d.messages {
        out.push_str(&format!("[{}] {}\n{}\n\n", sender_label(&m.sender), m.created_at, m.text));
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn render_html(d: &ConvDetail) -> String {
    let mut body = String::new();
    for m in &d.messages {
        let cls = if m.sender == "human" { "human" } else { "assistant" };
        body.push_str(&format!(
            "<div class=\"msg {cls}\"><div class=\"who\">{} <span class=\"ts\">{}</span></div><pre>{}</pre></div>\n",
            sender_label(&m.sender),
            m.created_at,
            esc(&m.text)
        ));
    }
    format!(
        r#"<!DOCTYPE html><html lang="zh"><head><meta charset="utf-8">
<title>{title}</title>
<style>
body{{font-family:-apple-system,"PingFang SC","Microsoft YaHei",sans-serif;max-width:860px;margin:0 auto;padding:32px 24px;color:#1d2129;background:#fff}}
h1{{font-size:22px}} .meta{{color:#86909c;font-size:13px;margin-bottom:24px}}
.msg{{margin:16px 0;padding:14px 16px;border-radius:10px}}
.msg.human{{background:#e8f1ff}} .msg.assistant{{background:#f7f8fa}}
.who{{font-weight:600;font-size:13px;margin-bottom:6px}} .ts{{color:#86909c;font-weight:400}}
pre{{white-space:pre-wrap;word-break:break-word;margin:0;font-family:inherit;font-size:14px;line-height:1.7}}
</style></head><body>
<h1>{title}</h1>
<div class="meta">来源: {source} · 消息数: {count} · {created} — {updated}</div>
{body}
</body></html>"#,
        title = esc(&d.meta.title),
        source = d.meta.source,
        count = d.meta.message_count,
        created = d.meta.created_at,
        updated = d.meta.updated_at,
        body = body
    )
}

fn safe_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if "/\\:*?\"<>|\n\r".contains(c) { '_' } else { c })
        .take(60)
        .collect();
    let t = cleaned.trim().to_string();
    if t.is_empty() { "untitled".into() } else { t }
}

fn render(d: &ConvDetail, format: &str) -> (String, &'static str) {
    match format {
        "txt" => (render_txt(d), "txt"),
        "html" => (render_html(d), "html"),
        _ => (render_markdown(d), "md"),
    }
}

#[tauri::command]
pub fn export_conversation(
    db: State<AppDb>,
    id: String,
    format: String,
    dest: String,
) -> Result<String, String> {
    let detail = get_conversation_inner(&db, &id)?;
    let (content, _) = render(&detail, &format);
    std::fs::write(&dest, content).map_err(|e| format!("写入失败: {e}"))?;
    Ok(dest)
}

#[tauri::command]
pub fn export_batch(
    db: State<AppDb>,
    ids: Vec<String>,
    format: String,
    dest_dir: String,
) -> Result<usize, String> {
    let dir = std::path::Path::new(&dest_dir);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut count = 0;
    for id in &ids {
        let detail = get_conversation_inner(&db, id)?;
        let (content, ext) = render(&detail, &format);
        let name = format!("{}-{}.{}", safe_filename(&detail.meta.title), &id[..8.min(id.len())], ext);
        std::fs::write(dir.join(name), content).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

fn get_conversation_inner(db: &State<AppDb>, id: &str) -> Result<ConvDetail, String> {
    let conn = db.0.lock().unwrap();
    let meta = conn
        .query_row(
            &format!("SELECT {META_COLS} FROM conversations WHERE id = ?1"),
            params![id],
            row_to_meta,
        )
        .map_err(|_| "会话不存在".to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, sender, text, created_at FROM messages WHERE conv_id = ?1 ORDER BY idx")
        .map_err(|e| e.to_string())?;
    let messages = stmt
        .query_map(params![id], |r| {
            Ok(Message {
                id: r.get(0)?,
                sender: r.get(1)?,
                text: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ConvDetail { meta, messages })
}

// ---------- 统计 ----------

#[derive(Serialize)]
pub struct SourceStat {
    pub source: String,
    pub conversations: i64,
    pub messages: i64,
    pub user_chars: i64,
    pub assistant_chars: i64,
}

#[derive(Serialize)]
pub struct MonthStat {
    pub month: String,
    pub messages: i64,
    pub user_messages: i64,
}

#[derive(Serialize)]
pub struct Stats {
    pub total_conversations: i64,
    pub total_messages: i64,
    pub user_messages: i64,
    pub user_chars: i64,
    pub assistant_chars: i64,
    pub by_source: Vec<SourceStat>,
    pub monthly: Vec<MonthStat>,
    pub longest: Vec<ConvMeta>,
}

#[tauri::command]
pub fn get_stats(db: State<AppDb>) -> Result<Stats, String> {
    let conn = db.0.lock().unwrap();

    let (total_conversations, user_chars, assistant_chars): (i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(user_chars),0), COALESCE(SUM(assistant_chars),0) FROM conversations",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let (total_messages, user_messages): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(sender='human'),0) FROM messages",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT source, COUNT(*), COALESCE(SUM(message_count),0),
                    COALESCE(SUM(user_chars),0), COALESCE(SUM(assistant_chars),0)
             FROM conversations GROUP BY source",
        )
        .map_err(|e| e.to_string())?;
    let by_source = stmt
        .query_map([], |r| {
            Ok(SourceStat {
                source: r.get(0)?,
                conversations: r.get(1)?,
                messages: r.get(2)?,
                user_chars: r.get(3)?,
                assistant_chars: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT substr(created_at, 1, 7) AS month, COUNT(*),
                    COALESCE(SUM(sender='human'),0)
             FROM messages WHERE created_at != ''
             GROUP BY month ORDER BY month",
        )
        .map_err(|e| e.to_string())?;
    let monthly = stmt
        .query_map([], |r| {
            Ok(MonthStat {
                month: r.get(0)?,
                messages: r.get(1)?,
                user_messages: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT {META_COLS} FROM conversations ORDER BY message_count DESC LIMIT 5"
        ))
        .map_err(|e| e.to_string())?;
    let longest = stmt
        .query_map([], row_to_meta)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(Stats {
        total_conversations,
        total_messages,
        user_messages,
        user_chars,
        assistant_chars,
        by_source,
        monthly,
        longest,
    })
}
