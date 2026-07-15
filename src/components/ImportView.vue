<script setup lang="ts">
import { ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Icon from "./Icon.vue";
import { api, type ImportResult } from "../api";

const emit = defineEmits<{ imported: [] }>();

const busy = ref("");
const log = ref<string[]>([]);

function pushResult(label: string, r: ImportResult) {
  log.value.unshift(
    `${new Date().toLocaleTimeString()} · ${label}：新增 ${r.imported}，更新 ${r.updated}，跳过 ${r.skipped}，共 ${r.messages} 条消息`
  );
}

async function importZip() {
  const path = await open({
    title: "选择 claude.ai 导出的 zip",
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (!path) return;
  busy.value = "zip";
  try {
    const r = await api.importClaudeZip(path as string);
    pushResult("Claude 网页导出包", r);
    emit("imported");
  } catch (e) {
    log.value.unshift(`❌ 导入失败：${e}`);
  } finally {
    busy.value = "";
  }
}

async function importCode() {
  busy.value = "code";
  try {
    const r = await api.importClaudeCode();
    pushResult("本机 Claude Code", r);
    emit("imported");
  } catch (e) {
    log.value.unshift(`❌ 导入失败：${e}`);
  } finally {
    busy.value = "";
  }
}

async function importFile() {
  const path = await open({
    title: "选择数据文件（JSON / CSV）",
    filters: [{ name: "数据文件", extensions: ["json", "csv"] }],
  });
  if (!path) return;
  busy.value = "file";
  try {
    const r = await api.importDataFile(path as string);
    pushResult("数据文件", r);
    emit("imported");
  } catch (e) {
    log.value.unshift(`❌ 导入失败：${e}`);
  } finally {
    busy.value = "";
  }
}

async function exportLibrary() {
  const dest = await save({
    title: "全库导出为 JSON 备份",
    defaultPath: "lighthistory-backup.json",
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!dest) return;
  busy.value = "export";
  try {
    const n = await api.exportLibrary(dest as string);
    log.value.unshift(`${new Date().toLocaleTimeString()} · 全库导出完成：${n} 个会话 → ${dest}`);
  } catch (e) {
    log.value.unshift(`❌ 导出失败：${e}`);
  } finally {
    busy.value = "";
  }
}

async function backupDb() {
  const dest = await save({
    title: "备份数据库文件",
    defaultPath: "lighthistory.db",
    filters: [{ name: "SQLite", extensions: ["db"] }],
  });
  if (!dest) return;
  busy.value = "db";
  try {
    await api.backupDb(dest as string);
    log.value.unshift(`${new Date().toLocaleTimeString()} · 数据库已备份 → ${dest}`);
  } catch (e) {
    log.value.unshift(`❌ 备份失败：${e}`);
  } finally {
    busy.value = "";
  }
}
</script>

<template>
  <div class="import-page">
    <h1>导入</h1>
    <p class="desc">把散落在各处的 AI 对话收进本地库，重复导入自动去重</p>

    <div class="cards">
      <div class="card import-card">
        <div class="card-icon"><Icon name="package" :size="22" /></div>
        <div class="card-title">Claude 网页导出包</div>
        <div class="card-desc">
          claude.ai → Settings → Privacy → Export data，导出的 zip 直接导入，无需解压
        </div>
        <button class="btn btn-primary" :disabled="!!busy" @click="importZip">
          {{ busy === "zip" ? "导入中…" : "选择 zip 文件" }}
        </button>
      </div>

      <div class="card import-card">
        <div class="card-icon"><Icon name="terminal" :size="22" /></div>
        <div class="card-title">本机 Claude Code 会话</div>
        <div class="card-desc">
          自动扫描 ~/.claude/projects/ 下所有项目的会话，正文已被清理的从索引抢救摘要
        </div>
        <button class="btn btn-primary" :disabled="!!busy" @click="importCode">
          {{ busy === "code" ? "扫描导入中…" : "一键扫描导入" }}
        </button>
      </div>

      <div class="card import-card">
        <div class="card-icon"><Icon name="file" :size="22" /></div>
        <div class="card-title">数据文件（JSON / CSV）</div>
        <div class="card-desc">
          通用消息文件按联系人归组导入；LightHistory 备份 JSON 自动识别并恢复
        </div>
        <button class="btn btn-primary" :disabled="!!busy" @click="importFile">
          {{ busy === "file" ? "导入中…" : "选择文件" }}
        </button>
      </div>

      <div class="card import-card soon">
        <div class="card-icon"><Icon name="sparkles" :size="22" /></div>
        <div class="card-title">ChatGPT / Codex</div>
        <div class="card-desc">规划中：ChatGPT 官方导出包、Codex session 会话记录</div>
        <button class="btn" disabled>敬请期待</button>
      </div>
    </div>

    <h2 class="section-title">备份</h2>
    <div class="cards">
      <div class="card import-card">
        <div class="card-icon"><Icon name="database" :size="22" /></div>
        <div class="card-title">全库导出（JSON）</div>
        <div class="card-desc">
          所有会话连同来源、账号一起导出为单个 JSON，可在任何设备的 LightHistory 里恢复
        </div>
        <button class="btn btn-primary" :disabled="!!busy" @click="exportLibrary">
          {{ busy === "export" ? "导出中…" : "全库导出" }}
        </button>
      </div>

      <div class="card import-card">
        <div class="card-icon"><Icon name="shield" :size="22" /></div>
        <div class="card-title">备份数据库文件</div>
        <div class="card-desc">直接复制 SQLite 数据库本体，含全文索引，恢复时放回应用数据目录即可</div>
        <button class="btn btn-primary" :disabled="!!busy" @click="backupDb">
          {{ busy === "db" ? "备份中…" : "备份数据库" }}
        </button>
      </div>
    </div>

    <div v-if="log.length" class="card log-card">
      <div class="log-title">导入记录</div>
      <div v-for="(l, i) in log" :key="i" class="log-line">{{ l }}</div>
    </div>
  </div>
</template>

<style scoped>
.import-page {
  padding: 26px 28px;
  height: 100%;
  overflow-y: auto;
}
h1 {
  font-size: 24px;
  font-weight: 700;
  margin-bottom: 4px;
}
.desc {
  color: var(--text-3);
  margin-bottom: 22px;
  font-size: 13px;
}
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 14px;
  max-width: 920px;
}
.import-card {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: flex-start;
  transition: box-shadow 0.2s, transform 0.15s;
}
.import-card:hover:not(.soon) {
  box-shadow: var(--shadow);
  transform: translateY(-1px);
}
.import-card.soon {
  opacity: 0.65;
}
.card-icon {
  width: 44px;
  height: 44px;
  border-radius: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--primary);
  background: var(--primary-light);
}
.card-title {
  font-size: 15px;
  font-weight: 700;
}
.card-desc {
  font-size: 12.5px;
  color: var(--text-2);
  flex: 1;
}
.section-title {
  font-size: 16px;
  font-weight: 700;
  margin: 24px 0 12px;
}
.log-card {
  margin-top: 14px;
  max-width: 920px;
  padding: 18px 22px;
}
.log-title {
  font-weight: 700;
  margin-bottom: 8px;
  font-size: 14px;
}
.log-line {
  font-size: 12.5px;
  color: var(--text-2);
  padding: 5px 0;
  border-bottom: 1px solid var(--fill);
}
.log-line:last-child {
  border-bottom: none;
}
</style>
