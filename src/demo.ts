// 演示模式：URL 带 ?demo 时用演示数据替代 Tauri IPC，用于截图和网页预览。
// 不影响正常运行 —— 真实环境里 window.__TAURI_INTERNALS__ 已由 Tauri 注入。

const CONVS = [
  { id: "d1", source: "claude_web", account: "user@example.com", title: "从零设计一门编程语言的类型系统", project: "", created_at: "2026-06-02T09:12:00Z", updated_at: "2026-07-08T14:22:00Z", message_count: 86, user_chars: 12480, assistant_chars: 68210 },
  { id: "d2", source: "claude_web", account: "user@example.com", title: "长篇小说的叙事结构与节奏控制", project: "", created_at: "2026-05-18T13:33:58Z", updated_at: "2026-07-05T09:12:00Z", message_count: 62, user_chars: 8321, assistant_chars: 45210 },
  { id: "d3", source: "claude_code", account: "本机", title: "给 CLI 工具添加增量备份与断点续传", project: "/Users/dev/tools/backup-cli", created_at: "2026-06-28T07:11:50Z", updated_at: "2026-07-01T11:10:46Z", message_count: 44, user_chars: 3200, assistant_chars: 21000 },
  { id: "d4", source: "claude_web", account: "user@example.com", title: "苏轼的旷达是如何炼成的", project: "", created_at: "2026-04-02T08:00:00Z", updated_at: "2026-06-20T10:30:00Z", message_count: 38, user_chars: 5100, assistant_chars: 24800 },
  { id: "d5", source: "claude_code", account: "本机", title: "SQLite FTS5 中文分词方案调研与落地", project: "/Users/dev/apps/lighthistory", created_at: "2026-06-12T02:00:00Z", updated_at: "2026-06-15T08:30:00Z", message_count: 31, user_chars: 2400, assistant_chars: 15600 },
  { id: "d6", source: "claude_web", account: "user@example.com", title: "健身计划：从久坐程序员到半马完赛", project: "", created_at: "2026-03-10T08:00:00Z", updated_at: "2026-06-11T10:30:00Z", message_count: 27, user_chars: 3900, assistant_chars: 16800 },
  { id: "d7", source: "claude_code", account: "本机", title: "Tauri 2 多窗口与系统托盘实践", project: "/Users/dev/apps/menubar", created_at: "2026-05-30T07:11:50Z", updated_at: "2026-06-08T11:10:46Z", message_count: 24, user_chars: 1900, assistant_chars: 12400 },
  { id: "d8", source: "claude_web", account: "user@example.com", title: "如何给五岁的孩子解释「时间」", project: "", created_at: "2026-05-02T08:00:00Z", updated_at: "2026-05-28T10:30:00Z", message_count: 15, user_chars: 1200, assistant_chars: 7800 },
];

const MSGS = [
  { id: "m1", sender: "human", text: "我想从零设计一门小语言的类型系统，目标是既有 Hindley-Milner 的推导体验，又能渐进式地加类型标注。先帮我把设计空间理一理。", created_at: "2026-06-02T09:12:00Z" },
  { id: "m2", sender: "assistant", text: "好，这个设计空间可以沿三根轴展开：\n\n1. 推导强度：全局 HM 推导 ↔ 局部推导 + 显式标注\n2. 渐进性：unknown/any 的边界语义，运行时检查插在哪里\n3. 表达力：要不要泛型、行多态、效应系统\n\n先说结论：渐进式 + HM 是有张力的——HM 依赖全局统一，而渐进类型引入的 any 会让统一失去唯一解。主流做法是把 any 当作「双向兼容」的特殊类型，代价是推导结果不再是主类型。", created_at: "2026-06-02T09:13:20Z" },
  { id: "m3", sender: "human", text: "那如果我只在模块边界强制标注，模块内部用 HM 呢？", created_at: "2026-06-02T09:16:00Z" },
  { id: "m4", sender: "assistant", text: "这正是很多现代语言的折中——模块边界即类型边界：\n\n- 模块内部：封闭世界，HM 推导完全可用，主类型存在\n- 模块边界：显式签名，既是文档又是编译屏障\n- 跨模块调用：只看签名，不看实现，编译并行度也上来了\n\nOCaml 的 .mli、Rust 的 pub fn 签名要求，本质都是这个思路。你的语言如果定位是「脚本级手感 + 工程级约束」，这条路非常稳。", created_at: "2026-06-02T09:18:40Z" },
];

const STATS = {
  total_conversations: 327,
  total_messages: 8642,
  user_messages: 3821,
  user_chars: 486000,
  assistant_chars: 2870000,
  by_source: [
    { source: "claude_web", account: "user@example.com", conversations: 214, messages: 6120, user_chars: 392000, assistant_chars: 2210000 },
    { source: "claude_code", account: "本机", conversations: 113, messages: 2522, user_chars: 94000, assistant_chars: 660000 },
  ],
  monthly: [
    { month: "2025-10", messages: 240, user_messages: 105 },
    { month: "2025-11", messages: 380, user_messages: 168 },
    { month: "2025-12", messages: 310, user_messages: 132 },
    { month: "2026-01", messages: 520, user_messages: 224 },
    { month: "2026-02", messages: 690, user_messages: 301 },
    { month: "2026-03", messages: 780, user_messages: 342 },
    { month: "2026-04", messages: 850, user_messages: 371 },
    { month: "2026-05", messages: 1120, user_messages: 489 },
    { month: "2026-06", messages: 1480, user_messages: 645 },
    { month: "2026-07", messages: 960, user_messages: 418 },
  ],
  longest: CONVS.slice(0, 5),
};

export function installDemo() {
  if (!new URLSearchParams(location.search).has("demo")) return;
  if ((window as any).__TAURI_INTERNALS__) return;
  (window as any).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args: any) => {
      switch (cmd) {
        case "list_conversations":
          return CONVS;
        case "get_conversation":
          return { meta: CONVS.find((c) => c.id === args?.id) ?? CONVS[0], messages: MSGS };
        case "search":
          return [
            { conv_id: "d1", title: "从零设计一门编程语言的类型系统", source: "claude_web", account: "user@example.com", snippet: "主流做法是把 any 当作「双向兼容」的特殊[[类型]]，代价是推导结果…", msg_id: "m2", updated_at: "2026-07-08T14:22:00Z" },
            { conv_id: "d5", title: "SQLite FTS5 中文分词方案调研与落地", source: "claude_code", account: "本机", snippet: "CJK 逐字分词后短语匹配，[[类型]]安全由 Rust 侧保证…", msg_id: "m1", updated_at: "2026-06-15T08:30:00Z" },
          ];
        case "get_stats":
          return STATS;
        default:
          return null;
      }
    },
    transformCallback: (cb: any) => cb,
    metadata: {},
  };
}
