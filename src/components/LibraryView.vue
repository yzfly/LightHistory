<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { save, open as openDir } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import {
  api,
  fmtDate,
  fmtNum,
  senderLabel,
  sourceLabel,
  type ConvDetail,
  type ConvMeta,
  type SearchHit,
} from "../api";

const conversations = ref<ConvMeta[]>([]);
const detail = ref<ConvDetail | null>(null);
const sourceFilter = ref("");
const accountFilter = ref("");
const accounts = ref<string[]>([]);
const sort = ref("updated");
const query = ref("");
const hits = ref<SearchHit[] | null>(null);
const loading = ref(false);
const toast = ref("");
const highlightMsgId = ref("");
let searchTimer: ReturnType<typeof setTimeout> | undefined;

async function reload() {
  accounts.value = await api.listAccounts();
  if (accountFilter.value && !accounts.value.includes(accountFilter.value)) {
    accountFilter.value = "";
  }
  conversations.value = await api.listConversations(
    sourceFilter.value,
    accountFilter.value,
    sort.value
  );
  if (!detail.value && conversations.value.length) {
    openConv(conversations.value[0].id);
  }
}
defineExpose({ reload });

onMounted(reload);

async function openConv(id: string, msgId = "") {
  detail.value = await api.getConversation(id);
  highlightMsgId.value = msgId;
  if (msgId) {
    setTimeout(() => {
      document.getElementById("msg-" + msgId)?.scrollIntoView({ block: "center" });
    }, 50);
  } else {
    document.querySelector(".chat-scroll")?.scrollTo(0, 0);
  }
}

function onQueryInput() {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(async () => {
    const q = query.value.trim();
    if (!q) {
      hits.value = null;
      return;
    }
    loading.value = true;
    try {
      hits.value = await api.search(q);
    } finally {
      loading.value = false;
    }
  }, 250);
}

function clearSearch() {
  query.value = "";
  hits.value = null;
}

function renderSnippet(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\[\[/g, "<mark>")
    .replace(/\]\]/g, "</mark>");
}

function showToast(msg: string) {
  toast.value = msg;
  setTimeout(() => (toast.value = ""), 2500);
}

async function exportCurrent(format: string) {
  if (!detail.value) return;
  const ext = format === "markdown" ? "md" : format;
  const dest = await save({
    defaultPath: `${detail.value.meta.title.slice(0, 40).replace(/[/\\:*?"<>|]/g, "_")}.${ext}`,
    filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
  });
  if (!dest) return;
  await api.exportConversation(detail.value.meta.id, format === "md" ? "markdown" : format, dest);
  showToast(`已导出到 ${dest}`);
}

async function exportAll(format: string) {
  const dir = await openDir({ directory: true, title: "选择导出目录" });
  if (!dir) return;
  const ids = conversations.value.map((c) => c.id);
  const n = await api.exportBatch(ids, format, dir as string);
  showToast(`已批量导出 ${n} 个会话`);
}

function printPdf() {
  window.print();
}

const detailChars = computed(() => {
  if (!detail.value) return "";
  const m = detail.value.meta;
  return `我输入 ${fmtNum(m.user_chars)} 字 · Claude 输出 ${fmtNum(m.assistant_chars)} 字`;
});
</script>

<template>
  <div class="library">
    <!-- 左：会话列表卡片 -->
    <div class="list-pane card">
      <div class="list-head">
        <div class="search-wrap">
          <Icon name="search" :size="15" class="search-ic" />
          <input
            type="text"
            v-model="query"
            @input="onQueryInput"
            placeholder="搜索会话内容"
            class="search-input"
          />
          <button v-if="query" class="clear-btn" @click="clearSearch" title="清除">
            <Icon name="x" :size="13" />
          </button>
        </div>
        <div class="filters">
          <select v-model="sourceFilter" @change="reload">
            <option value="">全部来源</option>
            <option value="claude_web">Claude 网页</option>
            <option value="claude_code">Claude Code</option>
            <option value="im_chat">聊天记录</option>
          </select>
          <select v-if="accounts.length > 1" v-model="accountFilter" @change="reload" :title="'按账号筛选'">
            <option value="">全部账号</option>
            <option v-for="a in accounts" :key="a" :value="a">{{ a }}</option>
          </select>
          <select v-model="sort" @change="reload">
            <option value="updated">最近更新</option>
            <option value="created">创建时间</option>
            <option value="messages">消息数</option>
          </select>
        </div>
      </div>

      <!-- 搜索结果 -->
      <div v-if="hits !== null" class="scroll">
        <div class="search-meta">
          {{ loading ? "搜索中…" : `${hits.length} 条结果` }}
        </div>
        <div
          v-for="h in hits"
          :key="h.msg_id"
          class="conv-item"
          :class="{ active: detail?.meta.id === h.conv_id }"
          @click="openConv(h.conv_id, h.msg_id)"
        >
          <div class="conv-title">
            <span class="dot" :class="h.source"></span>{{ h.title }}
          </div>
          <div class="conv-snippet" v-html="renderSnippet(h.snippet)"></div>
        </div>
      </div>

      <!-- 会话列表 -->
      <div v-else class="scroll">
        <div class="search-meta">{{ conversations.length }} 个会话</div>
        <div
          v-for="c in conversations"
          :key="c.id"
          class="conv-item"
          :class="{ active: detail?.meta.id === c.id }"
          @click="openConv(c.id)"
        >
          <div class="conv-title">
            <span class="dot" :class="c.source"></span>{{ c.title || "（无标题）" }}
          </div>
          <div class="conv-sub">
            <span class="tag" :class="{ code: c.source === 'claude_code' }">{{
              sourceLabel(c.source)
            }}</span>
            <span>{{ c.message_count }} 条</span>
            <span>{{ fmtDate(c.updated_at).slice(0, 10) }}</span>
          </div>
        </div>
        <div v-if="!conversations.length" class="empty">
          <Icon name="database" :size="32" style="opacity: 0.25" />
          <div>还没有数据<br />去「导入」页把对话记录收进来吧</div>
        </div>
      </div>
    </div>

    <!-- 右：阅读视图卡片 -->
    <div class="detail-pane card" v-if="detail">
      <div class="detail-head">
        <div class="detail-title-wrap">
          <div class="detail-title">{{ detail.meta.title }}</div>
          <div class="detail-sub">
            {{ sourceLabel(detail.meta.source) }}
            <template v-if="detail.meta.account"> · {{ detail.meta.account }}</template>
            <template v-if="detail.meta.project"> · {{ detail.meta.project }}</template>
            · {{ detail.meta.message_count }} 条消息 · {{ detailChars }}
          </div>
        </div>
        <div class="detail-actions">
          <button class="btn" @click="exportCurrent('md')" title="导出 Markdown">MD</button>
          <button class="btn" @click="exportCurrent('txt')" title="导出纯文本">TXT</button>
          <button class="btn" @click="exportCurrent('html')" title="导出 HTML">HTML</button>
          <button class="btn btn-primary" @click="printPdf">
            <Icon name="printer" :size="14" />PDF
          </button>
          <button
            class="btn"
            @click="exportAll('markdown')"
            title="把当前筛选下的全部会话导出为 Markdown"
          >
            <Icon name="folder" :size="14" />批量
          </button>
        </div>
      </div>
      <div class="chat-scroll print-area">
        <div
          v-for="m in detail.messages"
          :key="m.id"
          :id="'msg-' + m.id"
          class="msg-row"
          :class="m.sender === 'human' ? 'human' : 'assistant'"
        >
          <div class="avatar" :class="m.sender === 'human' ? 'human' : 'assistant'">
            <Icon :name="m.sender === 'human' ? 'user' : 'bot'" :size="15" />
          </div>
          <div class="msg" :class="{ hl: m.id === highlightMsgId }">
            <div class="msg-who">
              {{ senderLabel(m.sender) }}
              <span class="msg-ts">{{ fmtDate(m.created_at) }}</span>
            </div>
            <pre class="msg-text">{{ m.text }}</pre>
          </div>
        </div>
      </div>
    </div>
    <div class="detail-pane card empty-detail" v-else>
      <div class="empty">
        <Icon name="message" :size="32" style="opacity: 0.25" />
        <div>选择左侧会话查看内容</div>
      </div>
    </div>

    <transition name="fade">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </transition>
  </div>
</template>

<style scoped>
.library {
  display: flex;
  height: 100%;
  gap: 14px;
  padding: 14px 14px 14px 0;
}
.list-pane {
  width: 316px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.list-head {
  padding: 16px 14px 12px;
}
.search-wrap {
  position: relative;
  margin-bottom: 10px;
}
.search-ic {
  position: absolute;
  left: 13px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--text-3);
  pointer-events: none;
}
.search-input {
  width: 100%;
  padding-left: 35px;
  padding-right: 32px;
}
.clear-btn {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  color: var(--text-3);
  padding: 4px;
  display: flex;
  border-radius: 50%;
}
.clear-btn:hover {
  color: var(--text);
  background: var(--fill-2);
}
.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.filters select {
  max-width: 100%;
}
.filters select {
  flex: 1;
  min-width: 0;
}
.scroll {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px 8px;
}
.search-meta {
  padding: 4px 10px 6px;
  font-size: 12px;
  color: var(--text-3);
}
.conv-item {
  padding: 11px 12px;
  cursor: pointer;
  border-radius: 14px;
  margin-bottom: 2px;
  transition: background 0.12s;
}
.conv-item:hover {
  background: var(--fill);
}
.conv-item.active {
  background: var(--primary-light);
}
.conv-title {
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
.dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  margin-right: 7px;
  vertical-align: 2px;
  background: var(--primary);
}
.dot.claude_code {
  background: #16a34a;
}
.conv-sub {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-top: 5px;
  font-size: 11px;
  color: var(--text-3);
}
.conv-snippet {
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
.detail-pane {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.detail-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 22px;
  border-bottom: 1px solid var(--fill);
}
.detail-title-wrap {
  min-width: 0;
}
.detail-title {
  font-size: 16px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.detail-sub {
  font-size: 12px;
  color: var(--text-3);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.detail-actions {
  display: flex;
  gap: 7px;
  flex-shrink: 0;
}
.chat-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 22px 26px 44px;
}
.msg-row {
  display: flex;
  gap: 10px;
  max-width: 840px;
  margin: 0 auto 18px;
}
.avatar {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
  margin-top: 2px;
}
.avatar.human {
  background: var(--primary-grad);
}
.avatar.assistant {
  background: #2b2b2b;
}
.msg {
  flex: 1;
  min-width: 0;
  padding: 12px 16px;
  border-radius: 16px;
  background: var(--bg);
}
.msg-row.human .msg {
  background: var(--primary-light);
}
.msg.hl {
  box-shadow: 0 0 0 2px var(--primary);
}
.msg-who {
  font-size: 12px;
  font-weight: 600;
  margin-bottom: 5px;
}
.msg-ts {
  color: var(--text-3);
  font-weight: 400;
  margin-left: 8px;
}
.msg-text {
  white-space: pre-wrap;
  word-break: break-word;
  font-family: inherit;
  font-size: 13.5px;
  line-height: 1.75;
}
.empty {
  padding: 60px 20px;
  text-align: center;
  color: var(--text-3);
  font-size: 13px;
  line-height: 2;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.empty-detail {
  align-items: center;
  justify-content: center;
}
.toast {
  position: fixed;
  bottom: 28px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(25, 25, 25, 0.92);
  color: #fff;
  padding: 11px 20px;
  border-radius: 999px;
  font-size: 13px;
  z-index: 99;
  max-width: 70vw;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  box-shadow: var(--shadow-lg);
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
