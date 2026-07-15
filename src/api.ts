import { invoke } from "@tauri-apps/api/core";

export interface ConvMeta {
  id: string;
  source: string;
  title: string;
  project: string;
  created_at: string;
  updated_at: string;
  message_count: number;
  user_chars: number;
  assistant_chars: number;
}

export interface Message {
  id: string;
  sender: string;
  text: string;
  created_at: string;
}

export interface ConvDetail {
  meta: ConvMeta;
  messages: Message[];
}

export interface SearchHit {
  conv_id: string;
  title: string;
  source: string;
  snippet: string;
  msg_id: string;
  updated_at: string;
}

export interface ImportResult {
  imported: number;
  updated: number;
  skipped: number;
  messages: number;
}

export interface SourceStat {
  source: string;
  conversations: number;
  messages: number;
  user_chars: number;
  assistant_chars: number;
}

export interface MonthStat {
  month: string;
  messages: number;
  user_messages: number;
}

export interface Stats {
  total_conversations: number;
  total_messages: number;
  user_messages: number;
  user_chars: number;
  assistant_chars: number;
  by_source: SourceStat[];
  monthly: MonthStat[];
  longest: ConvMeta[];
}

export const api = {
  listConversations: (source?: string, sort?: string) =>
    invoke<ConvMeta[]>("list_conversations", { source: source || null, sort: sort || null }),
  getConversation: (id: string) => invoke<ConvDetail>("get_conversation", { id }),
  search: (query: string) => invoke<SearchHit[]>("search", { query }),
  importClaudeZip: (path: string) => invoke<ImportResult>("import_claude_zip", { path }),
  importClaudeCode: () => invoke<ImportResult>("import_claude_code"),
  exportConversation: (id: string, format: string, dest: string) =>
    invoke<string>("export_conversation", { id, format, dest }),
  exportBatch: (ids: string[], format: string, destDir: string) =>
    invoke<number>("export_batch", { ids, format, destDir }),
  getStats: () => invoke<Stats>("get_stats"),
};

export const SOURCE_LABELS: Record<string, string> = {
  claude_web: "Claude 网页",
  claude_code: "Claude Code",
};

export function sourceLabel(s: string): string {
  return SOURCE_LABELS[s] ?? s;
}

export function fmtDate(iso: string): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso.slice(0, 16);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate()
  ).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export function fmtNum(n: number): string {
  if (n >= 100000000) return (n / 100000000).toFixed(2) + " 亿";
  if (n >= 10000) return (n / 10000).toFixed(1) + " 万";
  return String(n);
}
