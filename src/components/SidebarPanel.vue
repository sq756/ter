<script setup lang="ts">
import { ref, computed, reactive, onMounted } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  files: any[];
  currentPath: string;
  bgTabs: any[];
  skills: any[];
  lastActivityMap: Record<string, number>;
  cpuChartRef: any; // Keep for prop sync but we'll use local refs for DOM
  memChartRef: any;
  netChartRef: any;
  healthMode: string;
  currentNetSpeed: { up: string, down: string };
  extraStats: any;
  isAutoPilot: boolean;
  sftpHeight: number;
  isAuditMode: boolean;
}>();

const emit = defineEmits(['switch-tab', 'proc-context', 'update:isAutoPilot', 'audit-ui', 'switch-mode', 'run-skill', 'change-dir', 'view-history', 'open-trigger-settings', 'fast-access', 'morse-down', 'morse-up', 'morse-context', 'explorer-context', 'cycle-health-mode', 'skill-context', 'resize-sftp-start', 'reload-skills', 'update-audit-mode', 'refresh-explorer']);

// v2.11.2: Rotary Switch Logic Expanded
const moduleModes = {
  health: [
    { id: 'res', label: 'CPU/RAM', apply: () => emit('cycle-health-mode', 'resource') },
    { id: 'net', label: 'NETWORK', apply: () => emit('cycle-health-mode', 'network') },
    { id: 'io', label: 'DISK IO', apply: () => emit('cycle-health-mode', 'detail') }
  ],
  sftp: [
    { id: 'std', label: 'STANDARD', apply: () => emit('update-audit-mode', false) },
    { id: 'audit', label: 'AUDIT', apply: () => emit('update-audit-mode', true) },
    { id: 'hidden', label: 'DOTFILES', apply: () => console.log('TER_SYSTEM: Dotfiles toggle not implemented yet') }
  ],
  skills: [
    { id: 'run', label: 'EXECUTE', apply: () => console.log('TER_SYSTEM: Skill execution mode active') },
    { id: 'reload', label: 'RELOAD', apply: () => emit('reload-skills') }
  ],
  processes: [
    { id: 'all', label: 'ALL TASKS', apply: () => {} },
    { id: 'bg', label: 'BACKGROUND', apply: () => {} }
  ],
  history: [
    { id: 'jump', label: 'JUMP', apply: () => {} },
    { id: 'edit', label: 'EDIT', apply: () => {} }
  ]
};

const modeIndices = reactive({ health: 0, sftp: 0, skills: 0, processes: 0, history: 0 });

const rotateMode = (module: keyof typeof moduleModes, step: number) => {
  const modes = moduleModes[module];
  const len = modes.length;
  modeIndices[module] = (modeIndices[module] + step + len) % len;
  modes[modeIndices[module]].apply();
};

// v2.6.0: Agentic Interaction (Drag & Drop + Context Menu)
const draggedFile = ref<any>(null);

const onDragStart = (f: any) => {
  if (f.is_dir) return;
  draggedFile.value = f;
};

const onDropOnSkill = (skill: any) => {
  if (draggedFile.value) {
    emit('run-skill', { ...skill, context_file: draggedFile.value });
    draggedFile.value = null;
  }
};

const handleUpload = async () => {
  try {
    const selected = await open({ multiple: false });
    if (selected && typeof selected === 'string') {
      const fileName = selected.split(/[\\/]/).pop();
      const remotePath = (props.currentPath === '/' ? '' : props.currentPath) + '/' + fileName;
      await invoke('upload_file', { localPath: selected, remotePath });
      emit('refresh-explorer');
    }
  } catch (e) {
    console.error('Upload failed:', e);
  }
};

// v2.2.11: Track last visited directories for FAST ACCESS
const lastVisited = computed(() => {
  try {
    const saved = localStorage.getItem('ter_fast_access');
    const parsed = saved ? JSON.parse(saved) : [];
    if (Array.isArray(parsed)) return parsed.slice(0, 5);
  } catch (e) {}
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

const isTabActive = (id: string) => {
  const last = props.lastActivityMap[id] || 0;
  return (Date.now() - last) < 1000;
};

// Connect local refs to parent's ref objects for ECharts initialization
const cpuChartLocal = ref(null);
const memChartLocal = ref(null);
const netChartLocal = ref(null);

onMounted(() => {
  if (cpuChartLocal.value) props.cpuChartRef.value = cpuChartLocal.value;
  if (memChartLocal.value) props.memChartRef.value = memChartLocal.value;
  if (netChartLocal.value) props.netChartRef.value = netChartLocal.value;
});
</script>

<template>
  <aside class="side-bar" @contextmenu.prevent>
    <div class="sidebar-branding" @click="$emit('open-trigger-settings')" @contextmenu.prevent="$emit('header-context', {event: $event, module: 'branding'})" title="Click for System Settings">
      <div class="branding-text">TER // ADVANCED_TERMINAL</div>
      <div class="scanline"></div>
    </div>

    <div class="module sys-health" style="cursor: pointer;">
      <header class="header-with-action" @click.prevent="rotateMode('health', -1)" @contextmenu.prevent="rotateMode('health', 1)">
        <span>System Health</span>
        <span class="mode-badge">[{{ moduleModes.health[modeIndices.health].label }}]</span>
      </header>
      
      <!-- Resource Mode: CPU & RAM -->
      <div v-if="healthMode === 'resource'" class="chart-box">
        <div class="stat-item">
          <canvas ref="cpuChartLocal" width="100" height="40" class="mini-chart"></canvas>
          <span class="label">CPU</span>
        </div>
        <div class="stat-item">
          <canvas ref="memChartLocal" width="100" height="40" class="mini-chart"></canvas>
          <span class="label">RAM</span>
        </div>
      </div>

      <!-- Network Mode: Speeds -->
      <div v-else-if="healthMode === 'network'" class="net-box">
        <div class="speed-row">
          <span class="label">UP:</span> <span class="val">{{ currentNetSpeed.up }}</span>
        </div>
        <div class="speed-row">
          <span class="label">DOWN:</span> <span class="val">{{ currentNetSpeed.down }}</span>
        </div>
        <canvas ref="netChartLocal" width="200" height="40" class="net-chart"></canvas>
      </div>

      <!-- Detail Mode: Meta -->
      <div v-else class="detail-box">
        <div class="meta-row"><span class="label">GPU:</span> <span class="val">{{ extraStats.gpu }}</span></div>
        <div class="meta-row"><span class="label">UPT:</span> <span class="val">{{ extraStats.uptime }}</span></div>
        <div class="meta-row"><span class="label">IP:</span> <span class="val">{{ extraStats.ip }}</span></div>
        <div class="meta-row"><span class="label">DISK:</span> <span class="val">{{ extraStats.disk }}</span></div>
      </div>
    </div>

    <div class="module scroller processes">
      <header class="header-with-action" @click.prevent="rotateMode('processes', -1)" @contextmenu.prevent="rotateMode('processes', 1)">
        <span>Running Processes</span>
        <span class="mode-badge">[{{ moduleModes.processes[modeIndices.processes].label }}]</span>
      </header>
      <ul class="data-list">
        <li v-for="t in bgTabs" :key="t.id" @click="$emit('switch-tab', t.id)" @contextmenu.prevent="$emit('proc-context', {event: $event, tab: t})">
          <span class="icon">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 17h16M4 7h16M4 12h16"></path></svg>
          </span>
          <span class="name">{{ t.title }}</span>
          <span class="val active" :class="{ 'breathing': isTabActive(t.id) }">ACTIVE</span>
        </li>
        <li v-if="bgTabs.length === 0" class="empty-hint">No background tasks</li>
      </ul>
    </div>

    <div class="module scroller skills-hub">
      <header class="header-with-action" @click.prevent="rotateMode('skills', -1)" @contextmenu.prevent="rotateMode('skills', 1)">
        <span>Skill Hub</span>
        <span class="mode-badge">[{{ moduleModes.skills[modeIndices.skills].label }}]</span>
        <button class="header-btn" title="Configure AI Triggers" @click.stop="$emit('open-trigger-settings')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
        </button>
      </header>
      <ul class="data-list">
        <li v-for="s in skills" :key="s.id" 
          @click="$emit('run-skill', s)" 
          @contextmenu.prevent="$emit('skill-context', {event: $event, skill: s})"
          @dragover.prevent 
          @drop="onDropOnSkill(s)" 
          :title="s.description"
          class="skill-item">
          <span class="icon">
            <svg v-if="!s.icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path></svg>
            <span v-else>{{ s.icon }}</span>
          </span>
          <span class="name">{{ s.name }}</span>
          <span class="val">RUN</span>
        </li>
        <li v-if="skills.length === 0" class="empty-hint">No skills in .ter/skills.json</li>
      </ul>
    </div>

    <!-- v2.2.11: FAST ACCESS instead of Session History -->
    <div class="module scroller history">
      <header class="header-with-action" @click.prevent="rotateMode('history', -1)" @contextmenu.prevent="rotateMode('history', 1)">
        <span>FAST ACCESS</span>
        <span class="mode-badge">[{{ moduleModes.history[modeIndices.history].label }}]</span>
      </header>
      <ul class="data-list">
        <li v-for="path in lastVisited" :key="path" 
            @click="onFastAccessClick(path)" 
            @contextmenu.prevent="$emit('explorer-context', { e: $event, file: { name: path, is_dir: true, path: path } })">
          <span class="icon">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"></path></svg>
          </span>
          <span class="name">{{ path.split('/').pop() || '/' }}</span>
          <span class="val">GOTO</span>
        </li>
        <li v-if="lastVisited.length === 0" class="empty-hint">No recent paths</li>
      </ul>
    </div>

    <div class="module scroller explorer" :style="{ height: sftpHeight + 'px', minHeight: '150px', flex: 'none' }">
      <header class="header-with-action" @click.prevent="rotateMode('sftp', -1)" @contextmenu.prevent="rotateMode('sftp', 1)">
        <span>SFTP Explorer</span>
        <div class="header-actions">
           <button class="header-btn" title="Upload File" @click.stop="handleUpload">
             <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="17 8 12 3 7 8"></polyline><line x1="12" y1="3" x2="12" y2="15"></line></svg>
           </button>
           <span class="mode-badge">[{{ moduleModes.sftp[modeIndices.sftp].label }}]</span>
        </div>
      </header>
      <div class="current-path">{{ currentPath }}</div>
      <ul class="data-list" style="flex: 1;">
        <li @click="$emit('change-dir', '..')" @contextmenu.prevent="$emit('explorer-context', { e: $event, file: { name: '..', is_dir: true } })" class="file-item">
          <span class="file-icon">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
          </span>
          <span class="file-name">..</span>
        </li>
        <li v-for="f in sortedFiles" :key="f.name" 
          @click="onItemClick(f)" 
          @contextmenu.prevent="$emit('explorer-context', { e: $event, file: f })"
          draggable="true"
          @dragstart="onDragStart(f)"
          class="file-item">
          <span class="file-icon">
            <svg v-if="f.is_dir" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
            <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path><polyline points="13 2 13 9 20 9"></polyline></svg>
          </span>
          <span class="file-name">{{ f.name }}</span>
          <span v-if="isAuditMode" class="audit-info">[{{ f.is_dir ? '755' : '644' }}]</span>
        </li>
      </ul>
      <div class="resizable-handle" @mousedown="$emit('resize-sftp-start', $event)"></div>
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

.sidebar-branding {
  height: 40px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  background: rgba(34, 197, 94, 0.05);
  border-bottom: 1px solid #27272a;
  position: relative;
  overflow: hidden;
  cursor: pointer;
  transition: background 0.2s;
}

.sidebar-branding:hover {
  background: rgba(34, 197, 94, 0.1);
}

.branding-text {
  font-size: 10px;
  color: #22c55e;
  letter-spacing: 0.2em;
  font-family: 'JetBrains Mono', monospace;
  font-weight: bold;
  z-index: 1;
  text-shadow: 0 0 5px rgba(34, 197, 94, 0.3);
}

.scanline {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 2px;
  background: rgba(34, 197, 94, 0.2);
  box-shadow: 0 0 15px rgba(34, 197, 94, 0.5);
  animation: scan 3s infinite linear;
  pointer-events: none;
}

@keyframes scan {
  0% { transform: translateY(-100%); opacity: 0; }
  10% { opacity: 1; }
  90% { opacity: 1; }
  100% { transform: translateY(40px); opacity: 0; }
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

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
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
  display: flex;
  align-items: center;
}
.header-btn:hover {
  opacity: 1;
}

.mode-badge {
  font-size: 9px;
  color: #22c55e;
  font-family: 'JetBrains Mono', monospace;
  opacity: 0.8;
  pointer-events: none;
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
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto; 
  max-height: 30%;
}

.explorer {
  position: relative;
  border-bottom: 1px solid #27272a;
  display: flex;
  flex-direction: column;
}

.resizable-handle {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 4px;
  cursor: ns-resize;
  background: transparent;
  z-index: 10;
  transition: background 0.2s;
}

.resizable-handle:hover {
  background: #22c55e;
}

.sys-health { height: 160px; flex-shrink: 0; transition: height 0.2s; overflow: hidden; }
.chart-box { display: flex; gap: 10px; height: 60px; }
.stat-item { flex: 1; display: flex; flex-direction: column; gap: 4px; }
.mini-chart { height: 40px; background: #000; border: 1px solid #18181b; border-radius: 4px; }
.sys-health .label { font-size: 9px; color: #52525b; text-transform: uppercase; letter-spacing: 0.1em; }
.sys-health .val { font-size: 11px; color: #d4d4d8; font-family: 'JetBrains Mono', monospace; margin-left: auto; }

.net-box { display: flex; flex-direction: column; gap: 6px; }
.speed-row { display: flex; justify-content: space-between; }
.net-chart { height: 40px; background: #000; border: 1px solid #18181b; margin-top: 4px; border-radius: 4px; }

.detail-box { display: flex; flex-direction: column; gap: 4px; }
.meta-row { display: flex; justify-content: space-between; padding-bottom: 2px; border-bottom: 1px solid rgba(255,255,255,0.03); }
.detail-box .val { color: #22c55e; }

.data-list { 
  list-style: none; 
  padding: 0; 
  margin: 0; 
  overflow-y: auto !important;
  overflow-x: hidden;
}

.data-list::-webkit-scrollbar { width: 4px; }
.data-list::-webkit-scrollbar-thumb { background: #333; border-radius: 4px; }

.audit-info {
  margin-left: 8px;
  font-size: 10px;
  color: #ef4444; /* 危险红色或暗灰色 */
  font-family: 'JetBrains Mono', monospace;
  opacity: 0.8;
  white-space: nowrap;
}

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
}

.file-item:hover, .data-list li:hover {
  background: rgba(34, 197, 94, 0.1);
}

.file-item:active, .data-list li:active {
  transform: scale(0.98);
  opacity: 0.7;
}

.file-name, .data-list .name { 
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 0 1 auto;
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
.data-list .val.active.breathing { animation: breathe 0.8s infinite; }
@keyframes breathe {
  0% { opacity: 1; filter: brightness(1); }
  50% { opacity: 0.6; filter: brightness(1.5); }
  100% { opacity: 1; filter: brightness(1); }
}
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