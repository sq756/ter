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
  let loadedIds = new Set<string>();

  if (saved) {
    try {
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
        loadedIds.add("tab-1");
      }
    } catch (e) { 
      await createNewTab("Main Shell", false, "tab-1"); 
      loadedIds.add("tab-1");
    }
  } else if (terminalTabs.value.length === 0) { 
    await createNewTab("Main Shell", false, "tab-1"); 
    loadedIds.add("tab-1");
  }

  // --- SYNC REMOTE SESSIONS ---
  setTimeout(async () => {
    try {
      const remoteSessions = await invoke<string[]>('list_remote_tmux_sessions');
      console.log("[App] Remote sessions found:", remoteSessions);
      for (const s of remoteSessions) {
        if (!loadedIds.has(s) && (s.startsWith('tab-') || s === 'tab-1')) {
          console.log("[App] Auto-mounting remote session:", s);
          // Mount as background task
          await createNewTab(`Remote: ${s.substring(0, 8)}`, false, s);
          const t = terminalTabs.value.find(x => x.id === s);
          if (t) t.isBackground = true;
        }
      }
    } catch (e) { console.error("[App] Failed to sync remote sessions:", e); }
  }, 1500);

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
  }, 2000);
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
  
  // v2.6.0: Handle File Context (Drag & Drop)
  if (skill.context_file) {
    const f = skill.context_file;
    backendLogs.value.push(`[AGENT] Processing file: ${f.name} with skill: ${skill.name}`);
    
    // If it's a visual skill or just any skill, we can inject the path
    const fullPath = (currentPath.value === '/' ? '' : currentPath.value) + '/' + f.name;
    const cmd = `${skill.rpc || skill.trigger} "${fullPath}"\r\n`;
    if (activeTabId.value) {
      invoke('write_pty', { tabId: activeTabId.value, data: cmd });
    }
    return;
  }

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
  if (e.ctrlKey) isCtrlPressed.value = true;
  if (e.altKey) isAltPressed.value = true;
  if (e.shiftKey) isShiftPressed.value = true;

  if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; 
  // Ctrl+T for new tab
  if (e.ctrlKey && e.key.toLowerCase() === 't') {
    e.preventDefault();
    if (isConnected.value) createNewTab();
  }
};

const handleGlobalKeyUp = (e: KeyboardEvent) => {
  if (!e.ctrlKey) isCtrlPressed.value = false;
  if (!e.altKey) isAltPressed.value = false;
  if (!e.shiftKey) isShiftPressed.value = false;
};

onMounted(async () => {
  window.addEventListener('contextmenu', preventDefaultContextMenu);
  window.addEventListener('keydown', handleGlobalKeyDown);
  window.addEventListener('keyup', handleGlobalKeyUp);
  
  const st = localStorage.getItem('ter_active_triggers'); if (st) try { activeTriggers.value = JSON.parse(st); } catch(e){}
  const sm = localStorage.getItem('ter_macros'); if (sm) try { activeMacros.value = JSON.parse(sm); } catch(e){}
  
  unlistenLog = await listen<string>('backend-log', (e) => { 
    if (!isLogsPaused.value) {
      backendLogs.value.push(e.payload); 
      if (backendLogs.value.length > 500) backendLogs.value.shift(); 
    }
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
        :class="{ 'collapsed': !isSidebarOpen }"
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :lastActivityMap="lastActivityMap"
        :cpuChartRef="cpuChartRef" :memChartRef="memChartRef" :netChartRef="netChartRef"
        :healthMode="healthMode" :currentNetSpeed="currentNetSpeed" :extraStats="extraStats"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @explorer-context="onExplorerContextMenu" @cycle-health-mode="cycleHealthMode"
        @skill-context="onSkillContextMenu"
      />

      <main class="workspace" ref="workspaceRef">
        <!-- (Rest of modals, menus, etc.) -->
        <!-- Skill Settings Modal -->
        <div v-if="showSkillSettings" class="modal-overlay" @click.self="showSkillSettings = false">
          <div class="auth-card cyber-card">
            <h2 class="cyber-title">SKILL_CONFIG: {{ selectedSkill?.name }}</h2>
            <div class="cyber-subtitle">/// PARAMETER_ADJUSTMENT</div>
            <div class="skill-form">
              <label class="label">ID</label>
              <input :value="selectedSkill?.id" disabled class="cyber-input" />
              <label class="label">RPC_COMMAND</label>
              <input v-model="selectedSkill.rpc" class="cyber-input" />
              <label class="label">DESCRIPTION</label>
              <textarea v-model="selectedSkill.description" class="cyber-input" rows="3"></textarea>
            </div>
            <button @click="showSkillSettings = false" class="btn-primary">APPLY_CHANGES</button>
          </div>
        </div>
        <!-- Context Menu -->
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename Tab</div>
          <div class="menu-item" @click="copyTabIdAction">🆔 Copy ID</div>
          <div class="menu-item" @click="copyRuntimeEnv">🌍 Copy Env</div>
          <div class="menu-item" @click="generateRunReport">📊 Run Report</div>
          <div class="menu-item" @click="sendToBackground(contextMenuTabId)">🚀 Background</div>
          <div class="menu-divider"></div><div class="menu-item" @click="copySelectedText">📋 Copy</div><div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div><div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Force Close</div>
        </div>

        <!-- Explorer Context Menu -->
        <div v-if="showExplorerMenu" class="context-menu" :style="{ top: explorerMenuY + 'px', left: explorerMenuX + 'px' }">
          <header class="menu-header">FILE ACTIONS</header>
          <template v-if="selectedFile?.is_dir">
            <div class="menu-item" @click="explorerActionCd">📂 CD into folder</div>
            <div class="menu-item" @click="explorerActionUpload">📤 Upload to this dir</div>
            <div class="menu-item" @click="explorerActionCopyPath">📋 Copy Path</div>
          </template>
          <template v-else>
            <div class="menu-item" @click="explorerActionDownload">📥 Download File</div>
            <div class="menu-divider"></div>
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
                <header><span class="title">Cyber Logs</span> <span v-if="isLogsPaused" class="pause-hint">PAUSED</span></header>
                <div class="logs-container" @mouseenter="isLogsPaused = true" @mouseleave="isLogsPaused = false">
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
          <div class="status-left">
            <button class="status-btn sidebar-toggle" @click="isSidebarOpen = !isSidebarOpen" title="Toggle Sidebar">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><line x1="9" y1="3" x2="9" y2="21"></line></svg>
            </button>
            <div class="status-item node-info">
              <span class="node-dot pulse purple"></span>
              <span class="label">NODE:</span> <span class="val">{{ host }}</span>
            </div>
            <div class="status-divider"></div>
            <div class="status-item agent-info stealth-zone" @mousedown.prevent="handleMorseMouse" @wheel.prevent="handleMorseWheel" @contextmenu.prevent="onMorseMacro">
              <div class="tiny-dot" :class="{ 'active': isMorsePressed }"></div>
              <span class="label">AGENT:</span> <span class="val">ACTIVE</span>
            </div>
          </div>

          <div class="hotkey-bar">
            <template v-if="cyberMode === 0">
              <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\t' })">TAB</button>
              <button class="kb-pendant" :class="{ 'active': isCtrlPressed }" @click="isCtrlPressed = !isCtrlPressed">CTRL</button>
              <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\x03' })">C-C</button>
              <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\x1b' })">ESC</button>
            </template>
            <template v-else>
              <button class="kb-pendant accept" @click="invoke('write_pty', { tabId: activeTabId, data: '\r' })">ACCEPT</button>
              <button class="kb-pendant discard" @click="cyberMode = 0">DISCARD</button>
            </template>
          </div>

          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload(false)">📸 Audit</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">{{ cyberMode === 1 ? '🖥️' : '🌐' }} Web</button>
            <div class="status-toggle"><span>Auto</span><label class="mini-switch"><input type="checkbox" v-model="isAutoPilot" /><span class="slider"></span></label></div>
            <button class="status-btn lock-btn" @click="isLocked = true">🔒 LOCK</button>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
/* (Styles unchanged) */
.app-shell { height: 100vh; background: #000; color: #d4d4d8; font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace, 'Segoe UI Emoji', 'Noto Color Emoji'; overflow: hidden; border-radius: 8px; border: 1px solid #18181b; }

.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; min-width: 0; }

.context-menu { position: fixed; z-index: 1000000; background: #09090b; border: 1px solid #22c55e; padding: 4px; min-width: 160px; box-shadow: 0 10px 25px rgba(0,0,0,0.8); }
.menu-header { padding: 6px 12px; font-size: 9px; color: #166534; border-bottom: 1px solid #18181b; margin-bottom: 4px; }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-item.danger { color: #ef4444; }
.menu-item.danger:hover { background: #ef4444; color: #000; }
.menu-divider { height: 1px; background: #18181b; margin: 4px 0; }

.status-bar { height: 32px; background: rgba(0,0,0,0.9); backdrop-filter: blur(10px); border-top: 1px solid #18181b; color: #52525b; display: flex; justify-content: space-between; align-items: center; padding: 0 12px; font-size: 10px; z-index: 100; flex-shrink: 0; }

.status-left { display: flex; align-items: center; gap: 0; }
.status-item { display: flex; align-items: center; gap: 8px; padding: 0 10px; height: 32px; }
.status-divider { width: 1px; height: 16px; background: #27272a; margin: 0 4px; }
.status-item .label { color: #3f3f46; font-weight: bold; }
.status-item .val { color: #a1a1aa; }
.node-dot { width: 6px; height: 6px; border-radius: 50%; }
.node-dot.purple { background: #a855f7; box-shadow: 0 0 8px #a855f7; }

/* Dynamic Hotkey Bar */
.hotkey-bar { display: flex; align-items: center; gap: 4px; height: 100%; }
.kb-pendant { 
  background: rgba(39, 39, 42, 0.3); 
  border: 0.5px solid rgba(255,255,255,0.05); 
  color: #71717a; 
  font-family: 'JetBrains Mono', monospace; 
  font-size: 9px; 
  padding: 2px 8px; 
  border-radius: 4px; 
  cursor: pointer; 
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  text-transform: uppercase;
}
.kb-pendant:hover { background: rgba(255,255,255,0.05); color: #fff; box-shadow: 0 0 10px rgba(34, 197, 94, 0.2); }
.kb-pendant.active { background: #22c55e; color: #000; box-shadow: 0 0 15px #22c55e; border-color: transparent; }
.kb-pendant.accept { color: #22c55e; border-color: rgba(34, 197, 94, 0.3); }
.kb-pendant.discard { color: #ef4444; border-color: rgba(239, 68, 68, 0.3); }

/* Animation */
.key-slide-enter-active, .key-slide-leave-active { transition: all 0.3s ease; }
.key-slide-enter-from { opacity: 0; transform: translateX(20px); }
.key-slide-leave-to { opacity: 0; transform: translateX(-20px); }
.status-right { display: flex; align-items: center; gap: 12px; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; transition: all 0.1s; }
.tiny-dot.active { transform: scale(1.1); box-shadow: 0 0 8px #22c55e; filter: brightness(1.5); }
.stealth-zone { cursor: pointer; }
.stealth-zone:hover { background: rgba(34, 197, 94, 0.05); }

.lock-btn { color: #71717a !important; border: 1px solid #27272a !important; padding: 2px 8px !important; border-radius: 4px !important; }
.lock-btn:hover { border-color: #ef4444 !important; color: #ef4444 !important; background: rgba(239, 68, 68, 0.1) !important; }

.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; position: relative; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; background: #000; overflow: hidden; display: flex; flex-direction: column; }
.cyber-container { display: flex; flex-direction: column; height: 100%; flex: 1; overflow: hidden; }
.cyber-logs-view { flex: 0 0 30%; display: flex; flex-direction: column; background: #000; border-bottom: 1px solid #27272a; overflow: hidden; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; user-select: text !important; -webkit-user-select: text !important; }
.pause-hint { font-size: 9px; color: #ef4444; border: 1px solid #ef4444; padding: 0 4px; border-radius: 2px; margin-left: 10px; animation: blink 1s infinite; }
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
</style>
