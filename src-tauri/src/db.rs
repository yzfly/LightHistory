use rusqlite::Connection;
use std::path::Path;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            source TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            project TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT '',
            message_count INTEGER NOT NULL DEFAULT 0,
            user_chars INTEGER NOT NULL DEFAULT 0,
            assistant_chars INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conv_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            idx INTEGER NOT NULL,
            sender TEXT NOT NULL,
            text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conv_id, idx);

        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            seg_text,
            conv_id UNINDEXED,
            msg_id UNINDEXED,
            tokenize = 'unicode61'
        );
        "#,
    )?;
    migrate(&conn)?;
    Ok(conn)
}

/// 老库升级：补 account 列。
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(conversations)")?;
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;
    if !cols.iter().any(|c| c == "account") {
        conn.execute(
            "ALTER TABLE conversations ADD COLUMN account TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF |
        0xF900..=0xFAFF | 0x3040..=0x30FF | 0xAC00..=0xD7AF)
}

/// 在 CJK 字符之间插入空格，使 unicode61 分词器能按单字索引中文。
pub fn segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if is_cjk(c) {
            out.push(' ');
            out.push(c);
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// 把 FTS snippet 中分词插入的空格去掉，恢复中文可读性。
pub fn unsegment_snippet(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let prev_cjk = i > 0 && is_cjk(chars[i - 1]);
            let next_cjk = i + 1 < chars.len() && is_cjk(chars[i + 1]);
            if prev_cjk || next_cjk {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// 把用户查询转成 FTS5 查询：中文连续片段做短语匹配，其余按词 AND。
pub fn build_fts_query(query: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for token in query.split_whitespace() {
        let seg = segment(token);
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        let escaped = seg.replace('"', "");
        if escaped.contains(' ') {
            parts.push(format!("\"{}\"", escaped));
        } else {
            parts.push(format!("\"{}\"", escaped));
        }
    }
    parts.join(" AND ")
}
