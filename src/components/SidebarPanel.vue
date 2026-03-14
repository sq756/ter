<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue';
import VaultView from './VaultView.vue';

const props = defineProps<{
  files: any[];
  currentPath: string;
  bgTabs: any[];
  webviewInstances?: any[];
  activeWebviewId?: string | null;
  skills: any[];
  lastActivityMap: Record<string, number>;
  cpuChartRef: any;
  memChartRef: any;
  netChartRef: any;
  healthMode: string;
  currentNetSpeed: { up: string, down: string };
  extraStats: any;
  isAutoPilot: boolean;
  isSafeMode: boolean;
  sftpHeight: number;
  slots: string[];
  isLogsOverlay: boolean;
  logs: string[];
}>();

const emit = defineEmits(['switch-tab', 'proc-context', 'update:isAutoPilot', 'audit-ui', 'switch-mode', 'run-skill', 'change-dir', 'view-history', 'open-trigger-settings', 'fast-access', 'morse-down', 'morse-up', 'morse-context', 'explorer-context', 'cycle-health-mode', 'skill-context', 'header-context', 'resize-sftp-start', 'resize-charts', 'view-changed', 'switch-web', 'web-context']);

const activeView = ref<string>('OPS');
const activeLogsSubView = ref<'realtime' | 'vault'>('realtime');

onMounted(() => {
  window.addEventListener('switch-sidebar-view', (e: any) => {
    activeView.value = e.detail;
  });
});

watch(activeView, (nv) => {
  emit('view-changed', nv);
  if (nv === 'OPS') {
    nextTick(() => { emit('resize-charts'); });
  }
});

const cycleView = (direction: 1 | -1 = 1) => {
  const currentIndex = props.slots.indexOf(activeView.value);
  if (currentIndex === -1) {
    activeView.value = props.slots[0];
    return;
  }
  const nextIndex = (currentIndex + direction + props.slots.length) % props.slots.length;
  activeView.value = props.slots[nextIndex];
};

const handleWheel = (e: WheelEvent) => {
  if (e.shiftKey || (e.target as HTMLElement).closest('.view-switcher-safe')) {
    e.preventDefault();
    cycleView(e.deltaY > 0 ? 1 : -1);
  }
};

const draggedFile = ref<any>(null);
const onDragStart = (f: any) => { if (!f.is_dir) draggedFile.value = f; };
const onDropOnSkill = (skill: any) => {
  if (draggedFile.value) {
    emit('run-skill', { ...skill, context_file: draggedFile.value });
    draggedFile.value = null;
  }
};

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
  return [...baseFiles].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
});

const onItemClick = (f: any) => { if (f.is_dir) emit('change-dir', f.name); };
const onFastAccessClick = (path: string) => { emit('fast-access', path); };
const isTabActive = (id: string) => (Date.now() - (props.lastActivityMap[id] || 0)) < 1000;

// v2.11.44: Throttled Log Rendering
const throttledLogs = ref<string[]>([]);
let throttleId: any = null;

watch(() => props.logs, (newLogs) => {
  if (!throttleId) {
    throttleId = setTimeout(() => {
      throttledLogs.value = newLogs;
      throttleId = null;
    }, 100);
  }
}, { immediate: true });

// v2.11.33: Data protection for NaN values
const safeVal = (v: any) => (v === null || v === undefined || (typeof v === 'number' && isNaN(v))) ? '[ SCANNING... ]' : v;
</script>

<template>
  <aside class="side-bar" @contextmenu.prevent.stop @wheel="handleWheel">
    <div class="sidebar-branding" @click="$emit('open-trigger-settings')">
      <div class="branding-text">TER // CYBER_DECK</div>
      <div class="scanline"></div>
    </div>

    <div class="view-switcher-safe">
      <button v-for="(s, idx) in slots" :key="s"
              :class="{ active: activeView === s, 'overlay-tab': idx === 2 && isLogsOverlay }" 
              @click="activeView = s">
        [{{ s }}]
      </button>
    </div>

    <!-- OPS View -->
    <div v-show="activeView === 'OPS'" class="safe-view-wrapper">
      <div class="module sys-health" @click="$emit('cycle-health-mode')" style="cursor: pointer;">
        <header>System Health ({{ healthMode.toUpperCase() }})</header>
        
        <!-- v2.11.33: Resource View with high-fidelity fallback bars -->
        <div v-if="healthMode === 'resource'" class="chart-box-enhanced">
          <div class="stat-row">
            <span class="label">CPU</span>
            <div class="cyber-bar-bg">
              <div class="cyber-bar-fill" :style="{ width: (safeVal(extraStats.cpu_raw) || 0) + '%' }"></div>
            </div>
            <canvas ref="cpuChartRef" width="100" height="30" class="mini-chart overlay-chart"></canvas>
          </div>
          <div class="stat-row">
            <span class="label">RAM</span>
            <div class="cyber-bar-bg">
              <div class="cyber-bar-fill blue" :style="{ width: (safeVal(extraStats.mem_raw) || 0) + '%' }"></div>
            </div>
            <canvas ref="memChartRef" width="100" height="30" class="mini-chart overlay-chart"></canvas>
          </div>
        </div>

        <div v-else-if="healthMode === 'network'" class="net-box">
          <div class="speed-row"><span class="label">UP:</span> <span class="val">{{ currentNetSpeed.up }}</span></div>
          <div class="speed-row"><span class="label">DOWN:</span> <span class="val">{{ currentNetSpeed.down }}</span></div>
          <canvas ref="netChartRef" width="200" height="40" class="net-chart"></canvas>
        </div>

        <!-- v2.11.33: Detail Mode Visual Hierarchies -->
        <div v-else class="detail-box">
          <div class="meta-row"><span class="label highlight">GPU:</span> <span class="val">{{ safeVal(extraStats.gpu) }}</span></div>
          <div class="meta-row"><span class="label highlight">UPT:</span> <span class="val">{{ safeVal(extraStats.uptime) }}</span></div>
          <div class="meta-row"><span class="label highlight">IP:</span> <span class="val">{{ safeVal(extraStats.ip) }}</span></div>
          <div class="meta-row"><span class="label highlight">DISK:</span> <span class="val">{{ safeVal(extraStats.disk) }}</span></div>
        </div>
      </div>
      <div class="module scroller processes">
        <header>Running Processes</header>
        <ul class="data-list">
          <!-- Terminal Tabs -->
          <li v-for="t in bgTabs" :key="t.id" @click="$emit('switch-tab', t.id)" @contextmenu.prevent.stop="$emit('proc-context', {event: $event, tab: t})">
            <span class="icon"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 17h16M4 7h16M4 12h16"></path></svg></span>
            <span class="name">{{ t.title }}</span>
            <span class="val active" :class="{ 'breathing': isTabActive(t.id) }">ACTIVE</span>
          </li>
          <!-- Webview Instances (v2.11.43) -->
          <li v-for="w in webviewInstances" :key="w.id" @click="$emit('switch-web', w.id)" @contextmenu.prevent.stop="$emit('web-context', {event: $event, web: w})">
            <span class="icon">🌍</span>
            <span class="name">{{ w.title || 'Web Task' }}</span>
            <span class="val active" :class="{ 'highlight': activeWebviewId === w.id }">WEB</span>
          </li>
        </ul>
      </div>
    </div>

    <!-- ARS View: Removed Settings Button -->
    <div v-show="activeView === 'ARS'" class="safe-view-wrapper safe-flex-wrapper">
      <div class="module scroller skills-hub">
        <header class="header-minimal">
          <span>Skill Hub</span>
        </header>
        <ul class="data-list">
          <li v-for="s in skills" :key="s.id" @click="$emit('run-skill', s)" @contextmenu.prevent.stop="$emit('skill-context', {event: $event, skill: s})" @dragover.prevent @drop="onDropOnSkill(s)">
            <span class="icon">{{ s.icon || '🛠️' }}</span><span class="name">{{ s.name }}</span><span class="val">RUN</span>
          </li>
        </ul>
      </div>
    </div>

    <!-- NAV View -->
    <div v-show="activeView === 'NAV'" class="safe-view-wrapper safe-flex-wrapper">
      <div class="module scroller history">
        <header>FAST ACCESS</header>
        <ul class="data-list">
          <li v-for="path in lastVisited" :key="path" @click="onFastAccessClick(path)">
            <span class="icon">🚀</span><span class="name">{{ path.split('/').pop() || '/' }}</span><span class="val">GOTO</span>
          </li>
        </ul>
      </div>
      <div class="module scroller explorer" :style="{ height: sftpHeight + 'px', flex: 'none' }">
        <header><span>SFTP Explorer</span><div class="current-path">{{ currentPath }}</div></header>
        <ul class="data-list">
          <li @click="$emit('change-dir', '..')" @contextmenu.prevent.stop="$emit('explorer-context', { e: $event, file: { name: '..', is_dir: true } })" class="file-item file-spacing">..</li>
          <li v-for="f in sortedFiles" :key="f.name" @click="onItemClick(f)" @contextmenu.prevent.stop="$emit('explorer-context', { e: $event, file: f })" draggable="true" @dragstart="onDragStart(f)" class="file-item file-spacing">
            <span class="file-icon">{{ f.is_dir ? '📂' : '📄' }}</span><span class="file-name">{{ f.name }}</span>
          </li>
        </ul>
        <div class="resizable-handle" @mousedown="$emit('resize-sftp-start', $event)"></div>
      </div>
    </div>

    <!-- LOGS View -->
    <div v-show="activeView === 'LOGS'" class="safe-view-wrapper safe-flex-wrapper">
      <div class="logs-nav">
        <button :class="{ active: activeLogsSubView === 'realtime' }" @click="activeLogsSubView = 'realtime'">LIVE</button>
        <button :class="{ active: activeLogsSubView === 'vault' }" @click="activeLogsSubView = 'vault'">VAULT</button>
      </div>
      
      <div v-if="activeLogsSubView === 'realtime'" class="module scroller full-height">
        <header>Cyber Intelligence Logs</header>
        <div class="log-stream">
          <div v-for="(log, i) in throttledLogs" :key="i" class="log-line">{{ log }}</div>
        </div>
      </div>
      <div v-else class="full-height">
        <VaultView />
      </div>
    </div>
  </aside>
</template>

<style scoped>
.view-switcher-safe { display: flex; height: 28px; background: #000; border-bottom: 1px solid #18181b; }
.view-switcher-safe button { flex: 1; background: transparent; border: none; color: #71717a; font-family: 'JetBrains Mono', monospace; font-size: 10px; cursor: pointer; transition: all 0.2s; font-weight: bold; }
.view-switcher-safe button.active { color: #22c55e; background: rgba(34, 197, 94, 0.1); border-bottom: 2px solid #22c55e; }
.view-switcher-safe button.overlay-tab { color: #a855f7; border-bottom-color: #a855f7; animation: border-pulse 1s infinite; }

@keyframes border-pulse { 0%, 100% { border-bottom-width: 2px; } 50% { border-bottom-width: 4px; } }

.side-bar { background: #09090b; width: 260px; height: 100%; display: flex; flex-direction: column; flex-shrink: 0; border-right: 1px solid #27272a; overflow: hidden; }
.sidebar-branding { height: 40px; display: flex; align-items: center; padding: 0 16px; background: rgba(34, 197, 94, 0.05); border-bottom: 1px solid #27272a; position: relative; overflow: hidden; cursor: pointer; }
.branding-text { font-size: 10px; color: #22c55e; letter-spacing: 0.2em; font-family: 'JetBrains Mono', monospace; font-weight: bold; z-index: 1; }
.safe-view-wrapper { display: flex; flex-direction: column; flex: 1; overflow: hidden; height: 100%; }
.safe-flex-wrapper .module.scroller { flex: 1 !important; max-height: none !important; height: 100% !important; }
.module { padding: 16px; border-bottom: 1px solid #27272a; }

/* v2.11.33: Unified Header Letter Spacing */
.module header { font-size: 11px; color: #71717a; margin-bottom: 12px; text-transform: uppercase; display: flex; justify-content: space-between; align-items: center; letter-spacing: 2px; }
.header-minimal { display: block; }

.scroller { min-height: 0; overflow-y: auto; }
.data-list { list-style: none; padding: 0; margin: 0; }
.data-list li, .file-item { display: flex; align-items: center; gap: 10px; padding: 6px 8px; margin-bottom: 2px; border-radius: 6px; cursor: pointer; color: #d4d4d8; font-size: 13px; transition: all 0.15s; }
.data-list li:hover { background: rgba(34, 197, 94, 0.08); color: #22c55e; }

/* v2.11.33: SFTP folder/icon spacing */
.file-spacing { gap: 18px !important; }

.log-line { font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; margin-bottom: 2px; }

.logs-nav { display: flex; background: #000; border-bottom: 1px solid #18181b; padding: 4px; gap: 4px; }
.logs-nav button { flex: 1; background: transparent; border: 1px solid transparent; color: #52525b; font-size: 9px; cursor: pointer; padding: 2px; border-radius: 2px; text-transform: uppercase; font-weight: bold; }
.logs-nav button.active { color: #22c55e; border-color: rgba(34, 197, 94, 0.2); background: rgba(34, 197, 94, 0.05); }
.scanline { position: absolute; top: 0; left: 0; width: 100%; height: 2px; background: rgba(34, 197, 94, 0.2); animation: scan 3s infinite linear; pointer-events: none; }
@keyframes scan { 0% { transform: translateY(-100%); } 100% { transform: translateY(40px); } }

/* v2.11.33: Cyber-style Progress Bars */
.sys-health { height: auto; }
.chart-box-enhanced { display: flex; flex-direction: column; gap: 12px; }
.stat-row { position: relative; display: flex; flex-direction: column; gap: 4px; }
.cyber-bar-bg { height: 4px; background: #18181b; border-radius: 2px; overflow: hidden; width: 100%; }
.cyber-bar-fill { height: 100%; background: #00ff9d; box-shadow: 0 0 8px #00ff9d; transition: width 0.5s ease; }
.cyber-bar-fill.blue { background: #3b82f6; box-shadow: 0 0 8px #3b82f6; }
.overlay-chart { position: absolute; top: 15px; left: 0; opacity: 0.4; pointer-events: none; }

.detail-box .label.highlight { color: #22c55e; font-weight: bold; opacity: 1; }
.detail-box .val { color: #a1a1aa; font-size: 10px; font-family: 'JetBrains Mono', monospace; }
.meta-row { display: flex; justify-content: space-between; border-bottom: 1px solid rgba(255,255,255,0.03); padding: 4px 0; }

.mini-chart { height: 30px; background: transparent; border: none; }
.breathing { animation: breathe 0.8s infinite; }
@keyframes breathe { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }
.resizable-handle { height: 4px; cursor: ns-resize; background: transparent; }
.resizable-handle:hover { background: #22c55e; }
</style>
