<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

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
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);
const backendLogs = ref<string[]>([]);
const skills = ref<any[]>([]);

const storageKey = (h: string) => `ter_tabs_${h.replace(/\s+/g, '_')}`;
let statsIntervalId: any = null;

// ==========================================
// --- DECOUPLED LOGIC ---
// ==========================================
const { 
  terminalTabs, activeTabId, backgroundTabs, 
  createNewTab, closeTab, sendToBackground, bringToForeground, renameTab 
} = useTabs(isConnected, backendLogs);

const {
  currentPath, realFiles, refreshExplorer, changeDir, onFastAccess
} = useExplorer(isConnected, activeTabId);

const {
  showExplorerMenu, explorerMenuX, explorerMenuY, selectedFile,
  onExplorerContextMenu, explorerActionCd, explorerActionCat, explorerActionVim, explorerActionCopyPath, explorerActionRun
} = useExplorerContextMenu(activeTabId, currentPath);

const {
  previewUrl, isWebviewLoading, refreshWebview, handleExtractDOM, onDomExtracted, captureAndUpload
} = useCyber(activeTabId, backendLogs);

const {
  showContextMenu, menuX, menuY, contextMenuTabId, hasErrorSelection,
  onTerminalContextMenu, copySelectedText, pasteFromClipboard, 
  renameTabAction, copyTabIdAction, diagnoseSelection, calculateMenuPosition
} = useContextMenu(activeTabId, renameTab);

const { 
  cpuChartRef, memChartRef, currentCpuUsage, initCharts, fetchStats 
} = useStats(currentAgentPort, agentToken);

const { 
  morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters,
  handleMorseMouse, handleMorseWheel, onMorseMacro
} = useMorse(activeTabId, calculateMenuPosition);

usePtyListener(
  activeTabId, connectionStatus, backendLogs, isAutoPilot, lastAutoPilotTime, 
  activeTriggers, captureAndUpload, refreshWebview, currentAgentPort
);

// ==========================================
// --- WATCHERS ---
// ==========================================
watch(terminalTabs, (val) => { 
  if (isConnected.value) localStorage.setItem(storageKey(host.value), JSON.stringify(val)); 
}, { deep: true });

watch(activeTriggers, (val) => { 
  localStorage.setItem('ter_active_triggers', JSON.stringify(val)); 
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
  console.log("[App] Connected to", hostLabel);
  try { agentToken.value = await invoke('get_agent_token'); } catch(e){}
  
  const saved = localStorage.getItem(storageKey(host.value));
  if (saved) {
    try {
      const ts = JSON.parse(saved); 
      if (!Array.isArray(ts) || ts.length === 0) {
        await createNewTab("Main Shell", false, "tab-1");
      } else {
        terminalTabs.value = ts;
        // Re-spawn PTYs for each saved tab
        for (const t of ts) {
          await createNewTab(t.title, false, t.id);
        }
        activeTabId.value = ts.find((t: any) => !t.isBackground)?.id || ts[0]?.id;
      }
    } catch (e) { await createNewTab("Main Shell", false, "tab-1"); }
  } else if (terminalTabs.value.length === 0) { 
    await createNewTab("Main Shell", false, "tab-1"); 
  }

  setTimeout(() => {
    console.log("[App] Refreshing explorer and skills...");
    refreshExplorer().then(() => console.log("[App] Explorer refreshed, files:", realFiles.value.length));
    invoke('load_remote_skills').then((s: any) => {
      skills.value = s;
      console.log("[App] Skills loaded:", s.length);
    }).catch((err)=> console.error("[App] Load skills failed", err));
    nextTick(() => {
      initCharts();
      if (statsIntervalId) clearInterval(statsIntervalId);
      statsIntervalId = setInterval(fetchStats, 3000);
    });
  }, 1000);
};

const runMacro = async (c: string) => { if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: c + '\n' }); showMorseMacro.value = false; };

const runSkill = async (skill: any) => {
  if (!isConnected.value) return;
  if (skill.context_requirement?.require_screenshot) {
    await captureAndUpload(true);
  }
  const rpc = skill.rpc || skill.trigger;
  if (rpc && activeTabId.value) {
    invoke('write_pty', { tabId: activeTabId.value, data: rpc.endsWith('\n') ? rpc : rpc + "\r\n" });
  }
};

let unlistenLog: any;
const preventDefaultContextMenu = (e: MouseEvent) => e.preventDefault();
const handleGlobalKeyDown = (e: KeyboardEvent) => { 
  if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; 
  // Ctrl+T for new tab
  if (e.ctrlKey && e.key.toLowerCase() === 't') {
    e.preventDefault();
    if (isConnected.value) createNewTab();
  }
};

onMounted(async () => {
  window.addEventListener('contextmenu', preventDefaultContextMenu);
  window.addEventListener('keydown', handleGlobalKeyDown);
  
  const st = localStorage.getItem('ter_active_triggers'); if (st) try { activeTriggers.value = JSON.parse(st); } catch(e){}
  const sm = localStorage.getItem('ter_macros'); if (sm) try { activeMacros.value = JSON.parse(sm); } catch(e){}
  
  unlistenLog = await listen<string>('backend-log', (e) => { 
    backendLogs.value.push(e.payload); 
    if (backendLogs.value.length > 500) backendLogs.value.shift(); 
  });
});

onUnmounted(() => {
  window.removeEventListener('contextmenu', preventDefaultContextMenu);
  window.removeEventListener('keydown', handleGlobalKeyDown);
  if (unlistenLog) unlistenLog();
  if (statsIntervalId) clearInterval(statsIntervalId);
});
</script>

<template>
  <div class="app-shell" @click="showContextMenu = false; showMorseMacro = false; showExplorerMenu = false">
    <CyberGate v-if="!isConnected" @connected="onConnected" />
    
    <div v-else class="main-view">
      <SettingsPanel :isOpen="showSettings" @close="showSettings = false" @update-macros="(m) => activeMacros = m" />
      <SidebarPanel 
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :cpuChartRef="cpuChartRef" :memChartRef="memChartRef"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @explorer-context="onExplorerContextMenu"
      />

      <main class="workspace" ref="workspaceRef">
        <!-- Context Menu -->
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename Tab</div><div class="menu-item" @click="copyTabIdAction">🆔 Copy ID</div><div class="menu-item" @click="sendToBackground(contextMenuTabId)">🚀 Background</div>
          <div class="menu-divider"></div><div class="menu-item" @click="copySelectedText">📋 Copy</div><div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div><div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Force Close</div>
        </div>

        <!-- Explorer Context Menu -->
        <div v-if="showExplorerMenu" class="context-menu" :style="{ top: explorerMenuY + 'px', left: explorerMenuX + 'px' }">
          <header class="menu-header">FILE ACTIONS</header>
          <template v-if="selectedFile?.is_dir">
            <div class="menu-item" @click="explorerActionCd">📂 CD into folder</div>
            <div class="menu-item" @click="explorerActionCopyPath">📋 Copy Path</div>
          </template>
          <template v-else>
            <div class="menu-item" @click="explorerActionRun">🚀 Run Executable</div>
            <div class="menu-item" @click="explorerActionCat">👁️ Cat File</div>
            <div class="menu-item" @click="explorerActionVim">✏️ Edit (Vim)</div>
            <div class="menu-divider"></div>
            <div class="menu-item" @click="explorerActionCd">📂 CD to parent dir</div>
            <div class="menu-item" @click="explorerActionCopyPath">📋 Copy Path</div>
          </template>
        </div>

        <!-- Morse Macros -->
        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
          <div class="menu-divider"></div><div class="menu-item" @click="showSettings = true">⚙️ Manage...</div>
        </div>

        <!-- Morse Preview -->
        <div v-if="morseSequence || morseText" class="morse-preview-overlay">
          <div class="sequence">{{ morseSequence }}</div>
          <div class="text">{{ morseText }}</div>
          <div class="candidates" v-if="possibleLetters">{{ possibleLetters }}</div>
        </div>

        <nav class="tool-bar"><div class="status-chip"><span class="pulse purple"></span> {{ host }}</div><div class="actions"><button @click="isLocked = true" class="btn-tool">Lock System</button></div></nav>
        
        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs 
              :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" 
              @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" 
              @terminal-context="onTerminalContextMenu" 
            />
          </section>

          <section class="cyber-pane" v-if="cyberMode !== 0">
            <div class="cyber-container">
              <div class="cyber-logs-view">
                <header><span class="title">Cyber Logs</span></header>
                <div class="logs-container">
                  <div v-for="(log, i) in backendLogs" :key="i" class="log-line">
                    <span class="line-num">{{ i + 1 }}</span> {{ log }}
                  </div>
                </div>
              </div>
              <div class="cyber-divider"></div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <div class="address-input-wrapper"><span class="secure-icon">🔒</span><input v-model="previewUrl" @keyup.enter="refreshWebview()" @focus="($event.target as HTMLInputElement).select()" class="address-bar-input" /></div>
                  <button class="refresh-btn" @click="refreshWebview()" :class="{ spinning: isWebviewLoading }">{{ isWebviewLoading ? '⏳' : '⚡' }}</button>
                  <button class="extract-btn" @click="handleExtractDOM">👁️ Extract</button>
                </nav>
                <CyberWebview :url="previewUrl" @dom-extracted="onDomExtracted" />
              </div>
            </div>
          </section>
        </div>

        <footer class="status-bar">
          <div class="status-left stealth-zone" @mousedown.prevent="handleMorseMouse" @wheel.prevent="handleMorseWheel" @contextmenu.prevent="onMorseMacro">
            <div class="tiny-dot" :class="{ 'active': isMorsePressed }"></div>
            <span class="item">1 | Agent: Active</span>
          </div>
          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload(false)">📸 Audit</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">{{ cyberMode === 1 ? '🖥️' : '🌐' }}</button>
            <div class="status-toggle"><span>Auto</span><label class="mini-switch"><input type="checkbox" v-model="isAutoPilot" /><span class="slider"></span></label></div>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
/* (Styles unchanged) */
.app-shell { height: 100vh; background: #000; color: #d4d4d8; font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace, 'Segoe UI Emoji', 'Noto Color Emoji'; overflow: hidden; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; min-width: 0; }

.context-menu { position: fixed; z-index: 1000000; background: #09090b; border: 1px solid #22c55e; padding: 4px; min-width: 160px; box-shadow: 0 10px 25px rgba(0,0,0,0.8); }
.menu-header { padding: 6px 12px; font-size: 9px; color: #166534; border-bottom: 1px solid #18181b; margin-bottom: 4px; }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-item.danger { color: #ef4444; }
.menu-item.danger:hover { background: #ef4444; color: #000; }
.menu-divider { height: 1px; background: #18181b; margin: 4px 0; }

.status-bar { height: 28px; background: #000; border-top: 1px solid #18181b; color: #52525b; display: flex; justify-content: space-between; align-items: center; padding: 0 12px; font-size: 10px; z-index: 100; flex-shrink: 0; }
.status-right { display: flex; align-items: center; }
.status-right > * { margin-left: 15px !important; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; transition: all 0.1s; }
.tiny-dot.active { transform: scale(1.1); box-shadow: 0 0 8px #22c55e; filter: brightness(1.5); }
.stealth-zone { display: flex; align-items: center; gap: 8px; cursor: pointer; height: 100%; padding: 0 5px; }
.stealth-zone:hover { background: rgba(34, 197, 94, 0.05); }

.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; position: relative; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; background: #000; overflow: hidden; display: flex; flex-direction: column; }
.cyber-container { display: flex; flex-direction: column; height: 100%; flex: 1; overflow: hidden; }
.cyber-logs-view { flex: 0 0 30%; display: flex; flex-direction: column; background: #000; border-bottom: 1px solid #27272a; overflow: hidden; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; user-select: text; -webkit-user-select: text; }
.cyber-webview-wrapper { flex: 1; background: #000; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { height: 32px; border-bottom: 1px solid #27272a; display: flex; align-items: center; padding: 0 8px; gap: 8px; }
.address-input-wrapper { flex: 1; background: #050505; border: 1px solid #18181b; height: 24px; animation: breathing-border 3s infinite; display: flex; align-items: center; padding: 0 8px; }
@keyframes breathing-border { 0% { border-color: #18181b; } 50% { border-color: #22c55e; } 100% { border-color: #18181b; } }
.address-bar-input { background: transparent; border: none; color: #22c55e; font-size: 10px; width: 100%; outline: none; font-family: 'JetBrains Mono', monospace; }
.extract-btn { background: rgba(168, 85, 247, 0.1); border: 1px solid #a855f7; color: #a855f7; font-size: 10px; padding: 2px 8px; cursor: pointer; font-family: 'JetBrains Mono', monospace; }
.extract-btn:hover { background: rgba(168, 85, 247, 0.2); box-shadow: 0 0 10px #a855f7; }

.morse-preview-overlay { position: absolute; bottom: 40px; left: 10px; background: rgba(0, 0, 0, 0.9); border: 1px solid #22c55e; padding: 10px 20px; z-index: 1000; display: flex; flex-direction: column; align-items: center; pointer-events: none; }
.morse-preview-overlay .sequence { font-size: 24px; color: #22c55e; letter-spacing: 4px; }
.morse-preview-overlay .candidates { font-size: 9px; color: #166534; margin-top: 5px; }

.tool-bar { height: 36px; background: #000; border-bottom: 1px solid #18181b; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }
.btn-tool { background: transparent; border: 1px solid #27272a; color: #52525b; padding: 3px 10px; font-size: 10px; cursor: pointer; text-transform: uppercase; }
.btn-tool:hover { border-color: #ef4444; color: #ef4444; }
.status-chip { font-size: 11px; color: #52525b; display: flex; align-items: center; gap: 8px; }
.pulse.purple { width: 6px; height: 6px; background: #a855f7; border-radius: 50%; box-shadow: 0 0 5px #a855f7; }

.mini-switch { position: relative; display: inline-block; width: 24px; height: 12px; }
.mini-switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #27272a; transition: .4s; border-radius: 12px; }
.slider:before { position: absolute; content: ""; height: 8px; width: 8px; left: 2px; bottom: 2px; background-color: white; transition: .4s; border-radius: 50%; }
input:checked + .slider { background-color: #3b82f6; }
input:checked + .slider:before { transform: translateX(12px); }
.status-toggle { display: flex; align-items: center; gap: 8px; font-size: 10px; color: #52525b; }
.status-btn { background: transparent; border: none; color: #52525b; cursor: pointer; font-size: 10px; padding: 2px 6px; border-radius: 4px; transition: all 0.2s; }
.status-btn:hover { color: #fff; }
</style>
