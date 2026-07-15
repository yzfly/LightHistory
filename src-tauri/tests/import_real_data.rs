use lighthistory_lib::testing;

#[test]
fn import_and_query_real_data() {
    let tmp = std::env::temp_dir().join("lightchat_test.db");
    let _ = std::fs::remove_file(&tmp);
    let mut conn = testing::open_db(&tmp).expect("open db");

    // 1. 导入真实 zip
    let zip = "/Users/zephyr/Dev/Agents/ClaudeHistory/data-c6accbb3-8454-4ac0-bf63-65854166cefa-1775989765-589a4d9f-batch-0000.zip";
    if std::path::Path::new(zip).exists() {
        let r = testing::import_zip(&mut conn, zip).expect("import zip");
        println!("zip: imported={} updated={} skipped={} messages={}", r.imported, r.updated, r.skipped, r.messages);
        assert!(r.imported > 0, "应导入至少一个会话");

        // 重复导入应全部跳过
        let r2 = testing::import_zip(&mut conn, zip).expect("re-import zip");
        println!("re-import: imported={} updated={} skipped={}", r2.imported, r2.updated, r2.skipped);
        assert_eq!(r2.imported, 0);
        assert!(r2.skipped > 0);
    }

    // 2. 导入本机 Claude Code（存在才测）
    match testing::import_code(&mut conn) {
        Ok(r) => println!("claude_code: imported={} messages={}", r.imported, r.messages),
        Err(e) => println!("claude_code skipped: {e}"),
    }

    // 3. 中文全文搜索
    let hits = testing::search(&conn, "哲理").expect("search");
    println!("搜索「哲理」: {} 条", hits.len());
    for (title, snippet) in hits.iter().take(3) {
        println!("  - {} | {}", title, snippet.chars().take(60).collect::<String>());
    }
    assert!(!hits.is_empty(), "中文搜索应有结果");
    assert!(hits[0].1.contains("[["), "snippet 应含高亮标记");

    // 4. 统计
    let (convs, msgs, user_chars) = testing::quick_stats(&conn).expect("stats");
    println!("统计: {convs} 会话, {msgs} 消息, 用户输入 {user_chars} 字");
    assert!(convs > 0 && msgs > 0 && user_chars > 0);
}
