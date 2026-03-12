<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  files: any[];
  currentPath: string;
  bgTabs: any[];
  skills: any[];
  cpuChartRef: any;
  memChartRef: any;
  isAutoPilot: boolean;
}>();

const emit = defineEmits(['switch-tab', 'proc-context', 'update:isAutoPilot', 'audit-ui', 'switch-mode', 'run-skill', 'change-dir', 'view-history', 'open-trigger-settings', 'fast-access']);

// v2.2.11: Track last visited directories for FAST ACCESS
const lastVisited = computed(() => {
  const saved = localStorage.getItem('ter_fast_access');
  if (saved) return JSON.parse(saved).slice(0, 5);
  return [];
});

const sortedFiles = computed(() => {
  const baseFiles = props.files.filter(f => f.name !== '..');
  const sorted = [...baseFiles].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
  return sorted;
});

const onItemClick = (f: any) => {
  if (f.is_dir) {
    emit('change-dir', f.name);
  }
};

const onFastAccessClick = (path: string) => {
  emit('fast-access', path);
};
</script>

<template>
  <aside class="side-bar">
    <div class="module sys-health" @click="$emit('switch-mode', 0)" style="cursor: pointer;">
      <header>System Health</header>
      <div class="chart-box">
        <div :ref="cpuChartRef" class="mini-chart"></div>
        <div :ref="memChartRef" class="mini-chart"></div>
      </div>
    </div>

    <div class="module scroller processes">
      <header>Running Processes</header>
      <ul class="data-list">
        <li v-for="t in bgTabs" :key="t.id" @click="$emit('switch-tab', t.id)" @contextmenu.prevent="$emit('proc-context', {event: $event, tab: t})">
          <span class="icon">📟</span>
          <span class="name">{{ t.title }}</span>
          <span class="val active">ACTIVE</span>
        </li>
        <li v-if="bgTabs.length === 0" class="empty-hint">No background tasks</li>
      </ul>
    </div>

    <div class="module scroller skills-hub">
      <header class="header-with-action">
        <span>Skill Hub</span>
        <button class="header-btn" title="Configure AI Triggers" @click="$emit('open-trigger-settings')">⚙️</button>
      </header>
      <ul class="data-list">
        <li v-for="s in skills" :key="s.id" @click="$emit('run-skill', s)" :title="s.description">
          <span class="icon">{{ s.icon || '🛠️' }}</span>
          <span class="name">{{ s.name }}</span>
          <span class="val">RUN</span>
        </li>
        <li v-if="skills.length === 0" class="empty-hint">No skills in .ter/skills.json</li>
      </ul>
    </div>

    <!-- v2.2.11: FAST ACCESS instead of Session History -->
    <div class="module scroller history">
      <header>FAST ACCESS</header>
      <ul class="data-list">
        <li v-for="path in lastVisited" :key="path" @click="onFastAccessClick(path)">
          <span class="icon">🚀</span>
          <span class="name">{{ path.split('/').pop() || '/' }}</span>
          <span class="val">GOTO</span>
        </li>
        <li v-if="lastVisited.length === 0" class="empty-hint">No recent paths</li>
      </ul>
    </div>

    <div class="module scroller explorer">
      <header>
        <span>SFTP Explorer</span>
        <div class="current-path">{{ currentPath }}</div>
      </header>
      <ul class="data-list">
        <li @click="$emit('change-dir', '..')" class="file-item">
          <span class="file-icon">📁</span>
          <span class="file-name">..</span>
        </li>
        <li v-for="f in sortedFiles" :key="f.name" @click="onItemClick(f)" class="file-item">
          <span class="file-icon">{{ f.is_dir ? '📁' : '📄' }}</span>
          <span class="file-name">{{ f.name }}</span>
        </li>
      </ul>
    </div>
  </aside>
</template>

<style scoped>
.side-bar { 
  background: #09090b; 
  width: 260px; 
  height: 100%; 
  display: flex; 
  flex-direction: column; 
  flex-shrink: 0; 
  border-right: 1px solid #27272a; 
}

.module { 
  padding: 16px; 
  border-bottom: 1px solid #27272a; 
}

.module header { 
  font-size: 11px;
  color: #71717a; 
  letter-spacing: 0.08em;
  font-weight: 600;
  margin-bottom: 12px;
  text-transform: uppercase;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-with-action {
  display: flex !important;
  justify-content: space-between;
  align-items: center;
}

.header-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  font-size: 10px;
  opacity: 0.6;
  transition: opacity 0.2s;
  padding: 0;
  line-height: 1;
}

.header-btn:hover {
  opacity: 1;
}

.current-path {
  font-size: 10px;
  color: #52525b;
  text-transform: none;
  letter-spacing: normal;
  font-weight: normal;
  word-break: break-all;
  opacity: 0.8;
}

.scroller { 
  flex: 0 0 auto; 
  overflow-y: auto; 
  max-height: 30%;
}

.explorer {
  flex: 1;
  max-height: none;
}

.sys-health { height: 140px; flex-shrink: 0; }
.chart-box { display: flex; gap: 10px; height: 40px; }
.mini-chart { flex: 1; height: 100%; background: #000; border: 1px solid #18181b; border-radius: 4px; }

.data-list { list-style: none; padding: 0; margin: 0; }

.file-item, .data-list li { 
  display: flex; 
  align-items: center;
  justify-content: flex-start;
  gap: 10px;
  padding: 6px 8px;
  margin-bottom: 2px;
  border-radius: 6px;
  cursor: pointer;
  color: #d4d4d8;
  font-size: 13px;
  transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
  border-bottom: none;
}

.file-item:hover, .data-list li:hover { 
  background: rgba(34, 197, 94, 0.08); 
  color: #22c55e; 
  transform: scale(1.02);
}

.file-item:active, .data-list li:active {
  transform: scale(0.98);
  opacity: 0.7;
}

.file-name, .data-list .name { 
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.file-icon, .data-list .icon { 
  opacity: 0.7;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
}

.data-list .val.active { color: #22c55e; font-weight: bold; font-size: 9px; }
.data-list .val { 
  color: #22c55e; 
  font-size: 8px; 
  border: 1px solid rgba(34, 197, 94, 0.4); 
  padding: 0px 4px; 
  border-radius: 3px;
  margin-left: auto;
}

.empty-hint { font-size: 10px; color: #3f3f46; text-align: center; padding: 10px; }

.skills-hub { max-height: 25%; }
</style>
