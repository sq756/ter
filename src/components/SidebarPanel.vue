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

const emit = defineEmits(['switch-tab', 'proc-context', 'update:isAutoPilot', 'audit-ui', 'switch-mode', 'run-skill', 'change-dir']);

const sortedFiles = computed(() => {
  // 1. Filter out '..' if it exists in the original list (we add it manually if not at root)
  const baseFiles = props.files.filter(f => f.name !== '..');
  
  // 2. Sort logic
  const sorted = [...baseFiles].sort((a, b) => {
    // Directories first
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    // Alphabetical case-insensitive
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });

  return sorted;
});

const onItemClick = (f: any) => {
  if (f.is_dir) {
    emit('change-dir', f.name);
  }
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
      <header>Skill Hub</header>
      <ul class="data-list">
        <li v-for="s in skills" :key="s.id" @click="$emit('run-skill', s)" :title="s.description">
          <span class="icon">{{ s.icon || '🛠️' }}</span>
          <span class="name">{{ s.name }}</span>
          <span class="val">RUN</span>
        </li>
        <li v-if="skills.length === 0" class="empty-hint">No skills in .ter/skills.json</li>
      </ul>
    </div>

    <div class="module scroller explorer">
      <header>
        <span>SFTP Explorer</span>
        <div class="current-path">{{ currentPath }}</div>
      </header>
      <ul class="data-list">
        <!-- Always '..' for navigation unless root? (Backend handles root logic) -->
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
  flex-direction: column;
  gap: 4px;
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

/* Refactored File Item */
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
  transition: background 0.15s ease, color 0.15s ease;
  border-bottom: none;
}

.file-item:hover, .data-list li:hover { 
  background: rgba(255, 255, 255, 0.08); 
  color: #ffffff; 
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
  color: #6366f1; 
  font-size: 8px; 
  border: 1px solid rgba(99, 102, 241, 0.4); 
  padding: 0px 4px; 
  border-radius: 3px;
  margin-left: auto;
}

.empty-hint { font-size: 10px; color: #3f3f46; text-align: center; padding: 10px; }

.skills-hub { max-height: 25%; }
</style>
