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
const connectionStatus = ref<'connected' | 'busy' | 'disconnected'>('disconnected');
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const showSettings = ref(false);
const activeMacros = ref<{name: string, cmd: string}[]>([]);

const isLocked = ref(false);
const isSidebarOpen = ref(true);
const isCtrlPressed = ref(false);
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

const { currentPath, realFiles, refreshExplorer, changeDir, onFastAccess } = useExplorer(isConnected, activeTabId);
const { showExplorerMenu, explorerMenuX, explorerMenuY, selectedFile, onExplorerContextMenu, explorerActionCd, explorerActionDownload } = useExplorerContextMenu(activeTabId, currentPath, refreshExplorer);
const { previewUrl, isWebviewLoading, refreshWebview, handleExtractDOM, onDomExtracted, captureAndUpload } = useCyber(activeTabId, backendLogs);
const { showContextMenu, menuX, menuY, contextMenuTabId, onTerminalContextMenu, copySelectedText, pasteFromClipboard, renameTabAction } = useContextMenu(activeTabId, renameTab, host, currentPath, currentAgentPort, terminalTabs);
const { cpuChartRef, memChartRef, netChartRef, currentCpuUsage, healthMode, currentNetSpeed, extraStats, initCharts, fetchStats, setHealthMode } = useStats(currentAgentPort, agentToken);

const cycleHealthMode = () => {
  const modes: any[] = ['resource', 'network', 'detail'];
  const next = modes[(modes.indexOf(healthMode.value) + 1) % modes.length];
  setHealthMode(next);
};

const { morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters, handleMorseMouse, handleMorseWheel, onMorseMacro } = useMorse(activeTabId, (x,y)=>({x,y}));

usePtyListener(activeTabId, connectionStatus, backendLogs, isAutoPilot, ref(0), activeTriggers, captureAndUpload, refreshWebview, handleExtractDOM, currentAgentPort, lastActivityMap);

// ==========================================
// --- METHODS ---
// ==========================================
const onConnected = async (hostLabel: string) => {
  host.value = hostLabel;
  isConnected.value = true;
  connectionStatus.value = 'connected';
  
  const saved = localStorage.getItem(storageKey(host.value));
  if (saved) {
    try {
      const ts = JSON.parse(saved); 
      if (Array.isArray(ts) && ts.length > 0) {
        terminalTabs.value = ts;
        for (const t of ts) { await createNewTab(t.title, false, t.id); }
        activeTabId.value = ts.find((t: any) => !t.isBackground)?.id || ts[0]?.id;
      } else { await createNewTab("Main Shell", false, "tab-1"); }
    } catch (e) { await createNewTab("Main Shell", false, "tab-1"); }
  } else { await createNewTab("Main Shell", false, "tab-1"); }

  setTimeout(() => {
    refreshExplorer();
    invoke('load_remote_skills').then((s: any) => { skills.value = s; });
    nextTick(() => {
      initCharts();
      if (statsIntervalId) clearInterval(statsIntervalId);
      statsIntervalId = setInterval(fetchStats, 3000);
    });
  }, 1000);
};

const runSkill = async (skill: any) => {
  if (!isConnected.value || !activeTabId.value) return;
  const rpc = skill.rpc || skill.trigger;
  if (rpc) invoke('write_pty', { tabId: activeTabId.value, data: rpc + "\r\n" });
};

const handleGlobalKeyDown = (e: KeyboardEvent) => { 
  if (e.ctrlKey) isCtrlPressed.value = true;
  if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; 
  if (e.ctrlKey && e.key.toLowerCase() === 't') { e.preventDefault(); if (isConnected.value) createNewTab(); }
};

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeyDown);
  window.addEventListener('keyup', (e) => { if (!e.ctrlKey) isCtrlPressed.value = false; });
  listen<string>('backend-log', (e) => { if (!isLogsPaused.value) { backendLogs.value.push(e.payload); if (backendLogs.value.length > 500) backendLogs.value.shift(); } });
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
        @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @explorer-context="onExplorerContextMenu" @cycle-health-mode="cycleHealthMode"
        @skill-context="(p:any) => { selectedSkill = p.skill; showSkillSettings = true; }"
      />

      <main class="workspace" @click.stop>
        <!-- Modals -->
        <div v-if="showSkillSettings" class="modal-overlay" @click.self="showSkillSettings = false">
          <div class="auth-card cyber-card">
            <h2 class="cyber-title">SKILL_CONFIG: {{ selectedSkill?.name }}</h2>
            <div class="skill-form">
              <label class="label">RPC_COMMAND</label>
              <input v-model="selectedSkill.rpc" class="cyber-input" />
            </div>
            <button @click="showSkillSettings = false" class="btn-primary">APPLY</button>
          </div>
        </div>

        <!-- Context Menus -->
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <div class="menu-item" @click="renameTabAction">✏️ Rename</div>
          <div class="menu-item" @click="sendToBackground(contextMenuTabId!)">🚀 Background</div>
          <div class="menu-divider"></div>
          <div class="menu-item" @click="copySelectedText">📋 Copy</div>
          <div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Close</div>
        </div>

        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" @terminal-context="onTerminalContextMenu" />
          </section>
          <section class="cyber-pane" :class="{ 'open': cyberMode === 1 }">
            <div class="cyber-container">
              <div class="logs-container">
                <div v-for="(log, i) in backendLogs" :key="i" class="log-line">{{ log }}</div>
              </div>
              <div class="webview-wrapper">
                <nav class="webview-address-bar">
                  <input v-model="previewUrl" @keyup.enter="refreshWebview()" class="address-bar-input" />
                </nav>
                <CyberWebview :url="previewUrl" @dom-extracted="onDomExtracted" />
              </div>
            </div>
          </section>
        </div>

        <!-- Unified Status Bar -->
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

          <div class="hotkey-bar">
            <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\t' })">TAB</button>
            <button class="kb-pendant" :class="{ 'active': isCtrlPressed }" @click="isCtrlPressed = !isCtrlPressed">CTRL</button>
            <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\x03' })">C-C</button>
            <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\x1b' })">ESC</button>
          </div>

          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload(false)">📸 Audit</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">{{ cyberMode === 1 ? '🖥️' : '🌐' }} Web</button>
            <div class="status-toggle">
              <span>Auto</span>
              <label class="mini-switch"><input type="checkbox" v-model="isAutoPilot" /><span class="slider"></span></label>
            </div>
            <button class="status-btn lock-btn" @click="isLocked = true">🔒 LOCK</button>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
.app-shell { height: 100vh; width: 100vw; background: #000; color: #d4d4d8; font-family: 'JetBrains Mono', monospace; overflow: hidden; display: flex; flex-direction: column; }
.main-view { display: flex; flex: 1; height: 100%; width: 100%; overflow: hidden; position: relative; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; position: relative; }
.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; display: none; flex-direction: column; }
.cyber-pane.open { display: flex; }
.cyber-container { display: flex; flex-direction: column; height: 100%; }
.logs-container { flex: 0 0 30%; overflow-y: auto; padding: 10px; background: #050505; border-bottom: 1px solid #27272a; font-size: 10px; }
.webview-wrapper { flex: 1; display: flex; flex-direction: column; }
.status-bar { height: 32px; background: #09090b; border-top: 1px solid #18181b; display: flex; justify-content: space-between; align-items: center; padding: 0 12px; font-size: 10px; flex-shrink: 0; z-index: 1000; }
.status-left, .status-right { display: flex; align-items: center; gap: 15px; }
.status-divider { width: 1px; height: 14px; background: #27272a; }
.node-dot { width: 6px; height: 6px; border-radius: 50%; background: #a855f7; display: inline-block; margin-right: 4px; box-shadow: 0 0 5px #a855f7; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; display: inline-block; margin-right: 4px; }
.tiny-dot.active { box-shadow: 0 0 8px #22c55e; }
.sidebar-toggle { color: #22c55e !important; cursor: pointer; display: flex; align-items: center; }
.status-btn { background: transparent; border: 1px solid transparent; color: #52525b; cursor: pointer; padding: 2px 6px; }
.status-btn:hover { color: #fff; border-color: #27272a; }
.lock-btn:hover { color: #ef4444 !important; border-color: rgba(239, 68, 68, 0.3) !important; }
.context-menu { position: fixed; z-index: 10000; background: #09090b; border: 1px solid #22c55e; padding: 4px; min-width: 140px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
.menu-item { padding: 6px 12px; font-size: 11px; cursor: pointer; color: #d4d4d8; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-divider { height: 1px; background: #18181b; margin: 4px 0; }
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.85); backdrop-filter: blur(5px); display: flex; align-items: center; justify-content: center; z-index: 20000; }
.cyber-card { background: #09090b; border: 1px solid #22c55e; padding: 30px; min-width: 400px; box-shadow: 0 0 30px rgba(34, 197, 94, 0.2); }
.address-bar-input { background: #000; border: 1px solid #27272a; color: #22c55e; padding: 4px 10px; font-size: 10px; outline: none; flex: 1; width: 100%; }

.mini-switch { position: relative; display: inline-block; width: 24px; height: 12px; }
.mini-switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #27272a; transition: .4s; border-radius: 12px; }
.slider:before { position: absolute; content: ""; height: 8px; width: 8px; left: 2px; bottom: 2px; background-color: white; transition: .4s; border-radius: 50%; }
input:checked + .slider { background-color: #3b82f6; }
input:checked + .slider:before { transform: translateX(12px); }
.status-toggle { display: flex; align-items: center; gap: 8px; font-size: 10px; color: #52525b; }

.kb-pendant { background: rgba(39, 39, 42, 0.3); border: 0.5px solid rgba(255,255,255,0.05); color: #71717a; font-size: 9px; padding: 2px 8px; border-radius: 4px; cursor: pointer; transition: all 0.2s; }
.kb-pendant.active { background: #22c55e; color: #000; box-shadow: 0 0 10px #22c55e; }
.hotkey-bar { display: flex; gap: 5px; }
</style>
