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

    // 3.5 账号维度
    let accounts = testing::list_accounts(&conn).expect("accounts");
    println!("账号: {:?}", accounts);
    assert!(!accounts.is_empty(), "zip 导入应带出账号");

    // 4. 统计
    let (convs, msgs, user_chars) = testing::quick_stats(&conn).expect("stats");
    println!("统计: {convs} 会话, {msgs} 消息, 用户输入 {user_chars} 字");
    assert!(convs > 0 && msgs > 0 && user_chars > 0);
}

#[test]
fn generic_import_and_backup_roundtrip() {
    let tmp = std::env::temp_dir().join("lighthistory_test2.db");
    let _ = std::fs::remove_file(&tmp);
    let mut conn = testing::open_db(&tmp).expect("open db");

    // 通用 JSON 消息数组
    let json_path = std::env::temp_dir().join("lh_generic.json");
    std::fs::write(&json_path, r#"[
      {"contact":"老友群","sender":"张三","content":"周末爬山吗","createTime":1751500800},
      {"contact":"老友群","isSend":1,"content":"走起","createTime":1751500900},
      {"contact":"李四","sender":"李四","content":"文档发我一下","createTime":1751501000}
    ]"#).unwrap();
    let r = testing::import_file(&mut conn, json_path.to_str().unwrap()).expect("generic json");
    println!("通用JSON: imported={} messages={}", r.imported, r.messages);
    assert_eq!(r.imported, 2);
    assert_eq!(r.messages, 3);

    // 通用 CSV
    let csv_path = std::env::temp_dir().join("lh_generic.csv");
    std::fs::write(&csv_path, "contact,sender,content,time,is_self\n工作群,王五,\"明早十点,评审\",1751502000,0\n工作群,我,收到,1751502100,1\n").unwrap();
    let r = testing::import_file(&mut conn, csv_path.to_str().unwrap()).expect("generic csv");
    println!("通用CSV: imported={} messages={}", r.imported, r.messages);
    assert_eq!(r.imported, 1);
    assert_eq!(r.messages, 2);

    // 全库导出 → 新库恢复
    let dump = std::env::temp_dir().join("lh_backup.json");
    let n = testing::export_library(&conn, dump.to_str().unwrap()).expect("export");
    assert_eq!(n, 3);
    let tmp3 = std::env::temp_dir().join("lighthistory_test3.db");
    let _ = std::fs::remove_file(&tmp3);
    let mut conn3 = testing::open_db(&tmp3).expect("open db3");
    let r = testing::import_file(&mut conn3, dump.to_str().unwrap()).expect("restore");
    println!("恢复: imported={} messages={}", r.imported, r.messages);
    assert_eq!(r.imported, 3);
    let (convs, msgs, _) = testing::quick_stats(&conn3).expect("stats");
    assert_eq!((convs, msgs), (3, 5));
}
