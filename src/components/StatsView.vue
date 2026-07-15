<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Icon from "./Icon.vue";
import { api, fmtNum, sourceLabel, type Stats } from "../api";

const stats = ref<Stats | null>(null);

onMounted(async () => {
  stats.value = await api.getStats();
});

const monthly = computed(() => {
  const m = stats.value?.monthly ?? [];
  return m.slice(-18);
});
const maxMonth = computed(() => Math.max(1, ...monthly.value.map((m) => m.messages)));

const tiles = computed(() => {
  const s = stats.value;
  if (!s) return [];
  return [
    { label: "会话总数", value: fmtNum(s.total_conversations), icon: "chats" },
    { label: "消息总数", value: fmtNum(s.total_messages), icon: "message" },
    { label: "我发出的消息", value: fmtNum(s.user_messages), icon: "user" },
    { label: "我输入的字数", value: fmtNum(s.user_chars), icon: "type" },
    { label: "Claude 输出字数", value: fmtNum(s.assistant_chars), icon: "bot" },
  ];
});
</script>

<template>
  <div class="stats-page" v-if="stats">
    <h1>统计</h1>
    <p class="desc">你和 AI 的全部对话资产，一目了然</p>

    <div class="tiles">
      <div v-for="t in tiles" :key="t.label" class="card tile">
        <div class="tile-icon"><Icon :name="t.icon" :size="18" /></div>
        <div class="tile-value">{{ t.value }}</div>
        <div class="tile-label">{{ t.label }}</div>
      </div>
    </div>

    <div class="row">
      <div class="card panel">
        <div class="panel-title">月度活跃</div>
        <div class="chart">
          <div
            v-for="m in monthly"
            :key="m.month"
            class="bar-col"
            :title="`${m.month}: ${m.messages} 条（我发 ${m.user_messages} 条）`"
          >
            <div class="bar-wrap">
              <div class="bar" :style="{ height: (m.messages / maxMonth) * 100 + '%' }">
                <div
                  class="bar-user"
                  :style="{ height: m.messages ? (m.user_messages / m.messages) * 100 + '%' : '0' }"
                ></div>
              </div>
            </div>
            <div class="bar-label">{{ m.month.slice(2) }}</div>
          </div>
          <div v-if="!monthly.length" class="empty">暂无数据</div>
        </div>
        <div class="legend">
          <span><i class="ldot ldot-total"></i>全部消息</span>
          <span><i class="ldot ldot-user"></i>我发出的</span>
        </div>
      </div>

      <div class="card panel narrow">
        <div class="panel-title">来源分布</div>
        <div v-for="s in stats.by_source" :key="s.source" class="src-row">
          <div class="src-head">
            <span class="src-name">{{ sourceLabel(s.source) }}</span>
            <span class="src-num">{{ s.conversations }} 会话 / {{ fmtNum(s.messages) }} 消息</span>
          </div>
          <div class="src-bar">
            <div
              class="src-fill"
              :class="{ code: s.source === 'claude_code' }"
              :style="{
                width: (s.conversations / Math.max(1, stats.total_conversations)) * 100 + '%',
              }"
            ></div>
          </div>
        </div>
      </div>
    </div>

    <div class="card panel">
      <div class="panel-title">最长的 5 个会话</div>
      <table class="top-table">
        <tbody>
          <tr v-for="(c, i) in stats.longest" :key="c.id">
            <td class="rank" :class="{ top: i < 3 }">{{ i + 1 }}</td>
            <td class="t-title">{{ c.title }}</td>
            <td>
              <span class="tag" :class="{ code: c.source === 'claude_code' }">{{
                sourceLabel(c.source)
              }}</span>
            </td>
            <td class="t-num">{{ c.message_count }} 条</td>
            <td class="t-num">我输入 {{ fmtNum(c.user_chars) }} 字</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.stats-page {
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
.tiles {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 14px;
  max-width: 1080px;
  margin-bottom: 14px;
}
.tile {
  padding: 20px;
  transition: box-shadow 0.2s, transform 0.15s;
}
.tile:hover {
  box-shadow: var(--shadow);
  transform: translateY(-1px);
}
.tile-icon {
  width: 38px;
  height: 38px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--primary);
  background: var(--primary-light);
  margin-bottom: 14px;
}
.tile-value {
  font-size: 26px;
  font-weight: 700;
  line-height: 1.15;
  letter-spacing: -0.5px;
}
.tile-label {
  font-size: 12px;
  color: var(--text-3);
  margin-top: 3px;
}
.row {
  display: flex;
  gap: 14px;
  max-width: 1080px;
  margin-bottom: 14px;
  align-items: stretch;
}
.panel {
  padding: 20px 22px;
  flex: 1;
  min-width: 0;
  max-width: 1080px;
}
.panel.narrow {
  flex: 0 0 320px;
}
.panel-title {
  font-weight: 700;
  font-size: 15px;
  margin-bottom: 16px;
}
.chart {
  display: flex;
  align-items: flex-end;
  gap: 7px;
  height: 180px;
}
.bar-col {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
}
.bar-wrap {
  flex: 1;
  display: flex;
  align-items: flex-end;
}
.bar {
  width: 100%;
  background: #ffe3cc;
  border-radius: 6px;
  min-height: 3px;
  overflow: hidden;
  display: flex;
  align-items: flex-end;
  transition: filter 0.15s;
}
.bar-col:hover .bar {
  filter: brightness(1.04) saturate(1.15);
}
.bar-user {
  width: 100%;
  background: var(--primary-grad);
  border-radius: 6px 6px 0 0;
}
.bar-label {
  font-size: 10px;
  color: var(--text-3);
  text-align: center;
  margin-top: 6px;
  white-space: nowrap;
}
.legend {
  display: flex;
  gap: 16px;
  margin-top: 12px;
  font-size: 12px;
  color: var(--text-2);
}
.ldot {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 4px;
  margin-right: 5px;
}
.ldot-total {
  background: #ffe3cc;
}
.ldot-user {
  background: var(--primary);
}
.src-row {
  margin-bottom: 16px;
}
.src-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 7px;
}
.src-name {
  font-size: 13px;
  font-weight: 600;
}
.src-num {
  font-size: 12px;
  color: var(--text-3);
}
.src-bar {
  height: 8px;
  background: var(--fill);
  border-radius: 999px;
  overflow: hidden;
}
.src-fill {
  height: 100%;
  background: var(--primary-grad);
  border-radius: 999px;
}
.src-fill.code {
  background: linear-gradient(135deg, #4ade80 0%, #16a34a 100%);
}
.top-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.top-table td {
  padding: 10px 10px 10px 0;
  border-bottom: 1px solid var(--fill);
}
.top-table tr:last-child td {
  border-bottom: none;
}
.rank {
  color: var(--text-3);
  font-weight: 700;
  width: 26px;
}
.rank.top {
  color: var(--primary);
}
.t-title {
  max-width: 380px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}
.t-num {
  color: var(--text-2);
  white-space: nowrap;
}
.empty {
  color: var(--text-3);
  font-size: 13px;
  margin: auto;
}
</style>
