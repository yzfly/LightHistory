<script setup lang="ts">
import { ref } from "vue";
import Icon from "./components/Icon.vue";
import LibraryView from "./components/LibraryView.vue";
import ImportView from "./components/ImportView.vue";
import StatsView from "./components/StatsView.vue";
import logoUrl from "./assets/logo.svg";

type Page = "library" | "stats" | "import";
const initial = new URLSearchParams(location.search).get("page");
const page = ref<Page>(
  initial === "stats" || initial === "import" ? initial : "library"
);
const libraryRef = ref<InstanceType<typeof LibraryView> | null>(null);

const NAV = [
  { key: "library", label: "会话库", icon: "chats" },
  { key: "stats", label: "统计", icon: "chart" },
  { key: "import", label: "导入", icon: "download" },
] as const;

function onImported() {
  page.value = "library";
  libraryRef.value?.reload();
}
</script>

<template>
  <div class="layout">
    <aside class="sidebar">
      <div class="logo">
        <img :src="logoUrl" alt="LightHistory" class="logo-img" />
        <span class="logo-text">LightHistory</span>
      </div>
      <nav>
        <button
          v-for="n in NAV"
          :key="n.key"
          :class="{ active: page === n.key }"
          @click="page = n.key"
        >
          <Icon :name="n.icon" :size="17" />
          {{ n.label }}
        </button>
      </nav>
      <div class="sidebar-foot">
        <Icon name="shield" :size="13" />
        <span>本地存储<br />数据不离开这台电脑</span>
      </div>
    </aside>
    <main class="main">
      <LibraryView v-show="page === 'library'" ref="libraryRef" />
      <StatsView v-if="page === 'stats'" />
      <ImportView v-show="page === 'import'" @imported="onImported" />
    </main>
  </div>
</template>

<style scoped>
.layout {
  display: flex;
  height: 100vh;
  background: var(--bg);
}
.sidebar {
  width: 196px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  padding: 22px 14px 18px;
}
.logo {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 10px 24px;
}
.logo-img {
  width: 34px;
  height: 34px;
}
.logo-text {
  font-size: 16px;
  font-weight: 700;
  letter-spacing: -0.2px;
  color: var(--text);
}
nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
nav button {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 11px 14px;
  background: transparent;
  color: var(--text-2);
  font-size: 14px;
  text-align: left;
  border-radius: 14px;
  font-weight: 500;
  transition: background 0.15s, color 0.15s;
}
nav button:hover {
  background: rgba(0, 0, 0, 0.04);
}
nav button.active {
  background: var(--card);
  color: var(--primary);
  font-weight: 600;
  box-shadow: var(--shadow);
}
.sidebar-foot {
  margin-top: auto;
  font-size: 11px;
  color: var(--text-3);
  padding: 0 10px;
  line-height: 1.5;
  display: flex;
  gap: 6px;
  align-items: flex-start;
}
.sidebar-foot svg {
  flex-shrink: 0;
  margin-top: 2px;
}
.main {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}
</style>
