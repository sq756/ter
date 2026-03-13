<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

const appWindow = getCurrentWindow();

// Components
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import CyberGate from './components/CyberGate.vue';

// Composables
import { useMorse } from './composables/useMorse';
import { useTabs } from './composables/useTabs';
import { useStats } from './composables/useStats';
import { useExplorer } from './composables/useExplorer';
import { useExplorerContextMenu } from './composables/useExplorerContextMenu';
import { useCyber } from './composables/useCyber';
import { useContextMenu } from './composables/useContextMenu';
import { usePtyListener } from './composables/usePtyListener';

// ==========================================
// --- GLOBAL STATE ---
// ==========================================
const isConnected = ref(false);
const host = ref('Remote Server');
const isAutoPilot = ref(false);
const lastAutoPilotTime = ref(0);
const connectionStatus = ref<'connected' | 'busy' | 'disconnected'>('disconnected');
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const showSettings = ref(false);
const activeMacros = ref<{name: string, cmd: string}[]>([]);

const isLocked = ref(false);
const isSidebarOpen = ref(true);
const isCtrlPressed = ref(false);
const isAltPressed = ref(false);
const isShiftPressed = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);
const backendLogs = ref<string[]>([]);
const isLogsPaused = ref(false);
const skills = ref<any[]>([]);

const storageKey = (h: string) => `ter_tabs_${h.replace(/\s+/g, '_')}`;
let statsIntervalId: any = null;

// ==========================================
// --- DECOUPLED LOGIC ---
// ==========================================
const { 
  terminalTabs, activeTabId, backgroundTabs, lastActivityMap,
  createNewTab, closeTab, sendToBackground, bringToForeground, renameTab 
} = useTabs(isConnected, backendLogs);

const {
  currentPath, realFiles, refreshExplorer, changeDir, onFastAccess
} = useExplorer(isConnected, activeTabId);

const {
  showExplorerMenu, explorerMenuX, explorerMenuY, selectedFile,
  onExplorerContextMenu, explorerActionCd, explorerActionCat, explorerActionVim, explorerActionCopyPath, explorerActionRun,
  explorerActionDownload, explorerActionUpload
} = useExplorerContextMenu(activeTabId, currentPath, refreshExplorer);

const {
  previewUrl, isWebviewLoading, refreshWebview, handleExtractDOM, onDomExtracted, captureAndUpload
} = useCyber(activeTabId, backendLogs);

const {
  showContextMenu, menuX, menuY, contextMenuTabId, hasErrorSelection,
  onTerminalContextMenu, copySelectedText, pasteFromClipboard, 
  renameTabAction, copyTabIdAction, copyRuntimeEnv, generateRunReport, diagnoseSelection, calculateMenuPosition
} = useContextMenu(activeTabId, renameTab, host, currentPath, currentAgentPort, terminalTabs);

const { 
  cpuChartRef, memChartRef, netChartRef, currentCpuUsage, 
  healthMode, currentNetSpeed, extraStats,
  initCharts, fetchStats, setHealthMode
} = useStats(currentAgentPort, agentToken);

const cycleHealthMode = () => {
  const modes: any[] = ['resource', 'network', 'detail'];
  const next = modes[(modes.indexOf(healthMode.value) + 1) % modes.length];
  setHealthMode(next);
};

const { 
  morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters,
  handleMorseMouse, handleMorseWheel, onMorseMacro
} = useMorse(activeTabId, calculateMenuPosition);

usePtyListener(
  activeTabId, connectionStatus, backendLogs, isAutoPilot, lastAutoPilotTime, 
  activeTriggers, captureAndUpload, refreshWebview, handleExtractDOM, currentAgentPort, lastActivityMap
);

// ==========================================
// --- WATCHERS ---
// ==========================================
watch(terminalTabs, (val) => { 
  if (isConnected.value) localStorage.setItem(storageKey(host.value), JSON.stringify(val)); 
}, { deep: true });

// ==========================================
// --- METHODS ---
// ==========================================
const viewHistory = async (originalTabId: string) => {
  const t = terminalTabs.value.find(x => x.id === originalTabId);
  const playbackId = await createNewTab(`Playback: ${t?.title || originalTabId}`, true);
  try {
    const logs = await invoke<number[][]>('get_terminal_logs', { tabId: originalTabId, limit: 1000 });
    for (const chunk of logs) { 
      terminalManager.write(playbackId, new Uint8Array(chunk)); 
      await new Promise(r => setTimeout(r, 20)); 
    }
  } catch (e) { terminalManager.write(playbackId, `\r\n[ERROR] History Fail: ${e}\r\n`); }
};

const onConnected = async (hostLabel: string) => {
  host.value = hostLabel;
  isConnected.value = true;
  connectionStatus.value = 'connected';
  
  const saved = localStorage.getItem(storageKey(host.value));
  let loadedIds = new Set<string>();

  try {
    if (saved) {
      const ts = JSON.parse(saved); 
      if (Array.isArray(ts) && ts.length > 0) {
        terminalTabs.value = ts;
        for (const t of ts) {
          await createNewTab(t.title, false, t.id);
          loadedIds.add(t.id);
        }
        activeTabId.value = ts.find((t: any) => !t.isBackground)?.id || ts[0]?.id;
      } else {
        await createNewTab("Main Shell", false, "tab-1");
      }
    } else {
      await createNewTab("Main Shell", false, "tab-1");
    }
  } catch (e) { 
    await createNewTab("Main Shell", false, "tab-1"); 
  }

  setTimeout(() => {
    refreshExplorer();
    invoke('load_remote_skills').then((s: any) => { skills.value = s; }).catch(() => {});
    nextTick(() => {
      initCharts();
      if (statsIntervalId) clearInterval(statsIntervalId);
      statsIntervalId = setInterval(fetchStats, 3000);
    });
  }, 1000);
};

const runMacro = async (c: string) => { if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: c + '\n' }); showMorseMacro.value = false; };

const showSkillSettings = ref(false);
const selectedSkill = ref<any>(null);

const onSkillContextMenu = (p: { event: MouseEvent, skill: any }) => {
  selectedSkill.value = p.skill;
  showSkillSettings.value = true;
};

const runSkill = async (skill: any) => {
  if (!isConnected.value) return;
  if (skill.context_file) {
    const f = skill.context_file;
    const fullPath = (currentPath.value === '/' ? '' : currentPath.value) + '/' + f.name;
    const cmd = `${skill.rpc || skill.trigger} "${fullPath}"\r\n`;
    if (activeTabId.value) invoke('write_pty', { tabId: activeTabId.value, data: cmd });
    return;
  }
  const rpc = skill.rpc || skill.trigger;
  if (rpc && activeTabId.value) invoke('write_pty', { tabId: activeTabId.value, data: rpc.endsWith('\n') ? rpc : rpc + "\r\n" });
};

const handleGlobalKeyDown = (e: KeyboardEvent) => { 
  if (e.ctrlKey) isCtrlPressed.value = true;
  if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; 
  if (e.ctrlKey && e.key.toLowerCase() === 't') {
    e.preventDefault();
    if (isConnected.value) createNewTab();
  }
};

const handleGlobalKeyUp = (e: KeyboardEvent) => {
  if (!e.ctrlKey) isCtrlPressed.value = false;
};

onMounted(async () => {
  window.addEventListener('contextmenu', (e) => e.preventDefault());
  window.addEventListener('keydown', handleGlobalKeyDown);
  window.addEventListener('keyup', handleGlobalKeyUp);
  
  const st = localStorage.getItem('ter_active_triggers'); if (st) try { activeTriggers.value = JSON.parse(st); } catch(e){}
  const sm = localStorage.getItem('ter_macros'); if (sm) try { activeMacros.value = JSON.parse(sm); } catch(e){}
  
  await listen<string>('backend-log', (e) => { 
    if (!isLogsPaused.value) {
      backendLogs.value.push(e.payload); 
      if (backendLogs.value.length > 500) backendLogs.value.shift(); 
    }
  });
});
</script>

<template>
  <div class="app-shell" @click="showContextMenu = false; showMorseMacro = false; showExplorerMenu = false">
    <CyberGate v-if="!isConnected" @connected="onConnected" />
    
    <div v-else class="main-view">
      <SettingsPanel :isOpen="showSettings" @close="showSettings = false" @update-macros="(m) => activeMacros = m" />
      
      <SidebarPanel 
        :class="{ 'collapsed': !isSidebarOpen }"
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :lastActivityMap="lastActivityMap"
        :cpuChartRef="cpuChartRef" :memChartRef="memChartRef" :netChartRef="netChartRef"
        :healthMode="healthMode" :currentNetSpeed="currentNetSpeed" :extraStats="extraStats"
        :isAutoPilot="isAutoPilot"
        @update:isAutoPilot="isAutoPilot = $event"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @explorer-context="onExplorerContextMenu" @cycle-health-mode="cycleHealthMode"
        @skill-context="onSkillContextMenu"
      />

      <main class="workspace">
        <div v-if="showSkillSettings" class="modal-overlay" @click.self="showSkillSettings = false">
          <div class="auth-card cyber-card">
            <h2 class="cyber-title">SKILL_CONFIG: {{ selectedSkill?.name }}</h2>
            <div class="skill-form">
              <label class="label">RPC_COMMAND</label>
              <input v-model="selectedSkill.rpc" class="cyber-input" />
              <label class="label">DESCRIPTION</label>
              <textarea v-model="selectedSkill.description" class="cyber-input" rows="3"></textarea>
            </div>
            <button @click="showSkillSettings = false" class="btn-primary">APPLY</button>
          </div>
        </div>

        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div class="menu-item" @click="renameTabAction">✏️ Rename</div>
          <div class="menu-item" @click="sendToBackground(contextMenuTabId)">🚀 Background</div>
          <div class="menu-divider"></div>
          <div class="menu-item" @click="copySelectedText">📋 Copy</div>
          <div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div>
          <div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Close</div>
        </div>

        <div v-if="showExplorerMenu" class="context-menu" :style="{ top: explorerMenuY + 'px', left: explorerMenuX + 'px' }">
          <header class="menu-header">FILE ACTIONS</header>
          <div class="menu-item" @click="explorerActionCd">📂 Open</div>
          <div class="menu-item" @click="explorerActionDownload">📥 Download</div>
        </div>

        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs 
              :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" 
              @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" 
              @terminal-context="onTerminalContextMenu" 
            />
          </section>
          <section class="cyber-pane" :class="{ 'open': cyberMode === 1 }">
            <div class="cyber-container">
              <div class="cyber-logs-view">
                <div class="logs-container">
                  <div v-for="(log, i) in backendLogs" :key="i" class="log-line">{{ log }}</div>
                </div>
              </div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <input v-model="previewUrl" @keyup.enter="refreshWebview()" class="address-bar-input" />
                  <button @click="refreshWebview()">⚡</button>
                </nav>
                <CyberWebview :url="previewUrl" @dom-extracted="onDomExtracted" />
              </div>
            </div>
          </section>
        </div>

        <footer class="status-bar">
          <div class="status-left">
            <button class="status-btn sidebar-toggle" @click.stop="isSidebarOpen = !isSidebarOpen">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><line x1="9" y1="3" x2="9" y2="21"></line></svg>
            </button>
            <div class="status-item"><span class="node-dot purple"></span> NODE: {{ host }}</div>
            <div class="status-divider"></div>
            <div class="status-item stealth-zone" @mousedown.prevent="handleMorseMouse" @wheel.prevent="handleMorseWheel" @contextmenu.prevent="onMorseMacro">
              <div class="tiny-dot" :class="{ 'active': isMorsePressed }"></div> AGENT: ACTIVE
            </div>
          </div>
          <div class="status-right">
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">🖥️/🌐</button>
            <button class="status-btn lock-btn" @click="isLocked = true">🔒 LOCK</button>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
.app-shell { height: 100vh; background: #000; color: #d4d4d8; font-family: 'JetBrains Mono', monospace; overflow: hidden; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; position: relative; }
.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; display: none; }
.cyber-pane.open { display: flex; }
.status-bar { height: 32px; background: #09090b; border-top: 1px solid #18181b; display: flex; justify-content: space-between; align-items: center; padding: 0 12px; font-size: 10px; flex-shrink: 0; }
.status-left, .status-right { display: flex; align-items: center; gap: 10px; }
.status-divider { width: 1px; height: 14px; background: #27272a; }
.node-dot { width: 6px; height: 6px; border-radius: 50%; background: #a855f7; display: inline-block; margin-right: 4px; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; display: inline-block; margin-right: 4px; }
.tiny-dot.active { box-shadow: 0 0 8px #22c55e; }
.sidebar-toggle { color: #22c55e !important; cursor: pointer; }
.status-btn { background: transparent; border: 1px solid transparent; color: #52525b; cursor: pointer; padding: 2px 6px; }
.status-btn:hover { color: #fff; border-color: #27272a; }
.lock-btn:hover { color: #ef4444 !important; }
.context-menu { position: fixed; z-index: 10000; background: #09090b; border: 1px solid #22c55e; padding: 4px; min-width: 140px; }
.menu-item { padding: 6px 12px; font-size: 11px; cursor: pointer; }
.menu-item:hover { background: #22c55e; color: #000; }
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 20000; }
.cyber-card { background: #09090b; border: 1px solid #22c55e; padding: 20px; min-width: 300px; }
.cyber-input { background: #000; border: 1px solid #27272a; color: #22c55e; padding: 8px; width: 100%; margin: 10px 0; }
</style>
