<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as echarts from 'echarts';

// Import Manager and Sub-components
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import CyberWebview from './components/CyberWebview.vue';
import SettingsPanel from './components/SettingsPanel.vue';

// ==========================================
// --- GLOBAL STATE ---
// ==========================================
const isConnected = ref(false);
const isConnecting = ref(false); 
const isMasterPasswordSet = ref(false);
const isAutoPilot = ref(false); 
const lastAutoPilotTime = ref(0);
const connectionStatus = ref<'connected' | 'busy' | 'disconnected'>('disconnected');
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const showTriggerConfig = ref(false);
const newTriggerStr = ref('');

// v2.2.14: Settings and Macros
const showSettings = ref(false);
const activeMacros = ref<{name: string, cmd: string}[]>([]);

// v2.2.13 Morse Engine State
const morseSequence = ref('');
const morseText = ref('');
const morseDownTime = ref(0);
const showMorseMacro = ref(false);
const morseTimer = ref<any>(null);

const morseMap: Record<string, string> = {
  '.-': 'A', '-...': 'B', '-.-.': 'C', '-..': 'D', '.': 'E', '..-.': 'F', '--.': 'G', '....': 'H', '..': 'I', '.---': 'J', '-.-': 'K', '.-..': 'L', '--': 'M', '-.': 'N', '---': 'O', '.--.': 'P', '--.-': 'Q', '.-.': 'R', '...': 'S', '-': 'T', '..-': 'U', '...-': 'V', '.--': 'W', '-..-': 'X', '-.--': 'Y', '--..': 'Z', '-----': '0', '.----': '1', '..---': '2', '...--': '3', '....-': '4', '.....': '5', '-....': '6', '--...': '7', '---..': '8', '----.': '9'
};

const isLocked = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);
const previewUrl = ref('http://localhost:5173');
const isWebviewLoading = ref(false);
const backendLogs = ref<string[]>([]);
const savedServers = ref<any[]>([]);
const host = ref('Remote Server');
const currentPath = ref('/');
const realFiles = ref<any[]>([]);
const skills = ref<any[]>([]);

// Context Menu State
const showContextMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const contextMenuTabId = ref<string | null>(null);
const hasErrorSelection = ref(false);

// Tabs State
const terminalTabs = ref<any[]>([]);
const activeTabId = ref<string | null>(null);
const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

// Persistence Logic
const storageKey = computed(() => `ter_tabs_${host.value.replace(/\s+/g, '_')}`);

// Watchers
watch(terminalTabs, (newTabs) => {
  if (isConnected.value) localStorage.setItem(storageKey.value, JSON.stringify(newTabs));
}, { deep: true });

watch(activeTriggers, (val) => {
  localStorage.setItem('ter_active_triggers', JSON.stringify(val));
}, { deep: true });

// ==========================================
// --- TAB MANAGEMENT ---
// ==========================================
const createNewTab = async (title = "Shell", skipPty = false, existingId?: string) => {
  const id = existingId || 'tab-' + Math.random().toString(36).substr(2, 9);
  terminalManager.setOnDataCallback(id, (tid, data) => {
    if (!skipPty && isConnected.value) invoke('write_pty', { tabId: tid, data });
  });
  terminalManager.getOrCreate(id);
  if (!skipPty && isConnected.value) {
    try { await invoke('spawn_new_pty', { tabId: id }); }
    catch (e) { backendLogs.value.push(`[ERROR] Failed to spawn PTY: ${e}`); }
  }
  if (!existingId) terminalTabs.value.push({ id, title, isBackground: false });
  activeTabId.value = id;
  return id;
};

const viewHistory = async (originalTabId: string) => {
  const originalTab = terminalTabs.value.find(t => t.id === originalTabId);
  const title = `Playback: ${originalTab?.title || originalTabId}`;
  const playbackId = await createNewTab(title, true);
  try {
    const logs = await invoke<number[][]>('get_terminal_logs', { tabId: originalTabId, limit: 1000 });
    for (const chunk of logs) {
      terminalManager.write(playbackId, new Uint8Array(chunk));
      await new Promise(r => setTimeout(r, 20));
    }
  } catch (e) { terminalManager.write(playbackId, `\r\n[ERROR] Failed to load history: ${e}\r\n`); }
};

const closeTab = (id: string) => {
  const index = terminalTabs.value.findIndex(t => t.id === id);
  if (index !== -1) {
    terminalTabs.value.splice(index, 1);
    terminalManager.remove(id);
    if (activeTabId.value === id) {
      activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null;
    }
  }
};

const copySelectedText = async () => {
  const id = contextMenuTabId.value || activeTabId.value;
  if (!id) return;
  const selection = terminalManager.getSelection(id);
  if (selection) await navigator.clipboard.writeText(selection);
  showContextMenu.value = false;
};

const pasteFromClipboard = async () => {
  const id = contextMenuTabId.value || activeTabId.value;
  if (!id) return;
  try {
    const text = await navigator.clipboard.readText();
    if (text) invoke('write_pty', { tabId: id, data: text });
  } catch (e) { console.error("Paste failed:", e); }
  showContextMenu.value = false;
};

const sendToBackground = () => {
  const targetId = contextMenuTabId.value || activeTabId.value;
  if (!targetId) return;
  const tab = terminalTabs.value.find(t => t.id === targetId);
  if (tab) {
    const selection = terminalManager.getSelection(tab.id).trim();
    tab.isBackground = true;
    tab.title = selection ? `Proc: ${selection.substring(0, 20)}...` : `Task: ${tab.id.substr(0, 5)}`;
    if (activeTabId.value === targetId) {
      activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null;
      if (!activeTabId.value) createNewTab("New Shell");
    }
  }
  showContextMenu.value = false;
};

const bringToForeground = (id: string) => {
  const tab = terminalTabs.value.find(t => t.id === id);
  if (tab) { tab.isBackground = false; activeTabId.value = id; }
};

const onTerminalContextMenu = (payload: { e: MouseEvent, id: string }) => {
  contextMenuTabId.value = payload.id;
  menuX.value = payload.e.clientX;
  menuY.value = payload.e.clientY;
  const selection = terminalManager.getSelection(payload.id);
  hasErrorSelection.value = selection.toLowerCase().includes('error') || 
                            selection.toLowerCase().includes('exception') ||
                            selection.includes('\x1b[31m');
  showContextMenu.value = true;
};

// ==========================================
// --- CORE LOGIC: SSH ---
// ==========================================
const connectWithId = async (id: string) => { 
  if (isConnecting.value) return; 
  isConnecting.value = true;
  connectionStatus.value = 'busy';
  const s = savedServers.value.find(s => s.id === id);
  if (s) host.value = s.label || s.host;
  invoke('connect_with_id', { id }).then(async () => {
    isConnecting.value = false;
    connectionStatus.value = 'connected';
    await onConnected();
  }).catch(e => {
    isConnecting.value = false;
    connectionStatus.value = 'disconnected';
    alert("Connection Failed: " + e);
  });
};

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  const saved = localStorage.getItem(storageKey.value);
  if (saved) {
    try {
      const tabs = JSON.parse(saved);
      terminalTabs.value = tabs;
      for (const t of tabs) await createNewTab(t.title, false, t.id);
      activeTabId.value = tabs.find((t: any) => !t.isBackground)?.id || tabs[0]?.id;
    } catch (e) { await createNewTab("Main Shell"); }
  } else { await createNewTab("Main Shell"); }

  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<any>('pty-data', (event) => {
    const { id, data } = event.payload;
    const bytes = new Uint8Array(data);
    const text = new TextDecoder().decode(bytes);

    if (connectionStatus.value === 'connected') {
      connectionStatus.value = 'busy';
      setTimeout(() => { if (isConnected.value) connectionStatus.value = 'connected'; }, 200);
    }

    if (isAutoPilot.value && id === activeTabId.value) {
      const plainText = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
      const isTmuxNoise = plainText.includes('tab-') && (plainText.length < 60 || plainText.includes('[') || plainText.includes(']'));
      const now = Date.now();
      if (!isTmuxNoise && (now - lastAutoPilotTime.value) > 500) {
        const linkMatch = plainText.match(/http:\/\/localhost:(\d+)/);
        if (linkMatch && linkMatch[1]) refreshWebview(`http://localhost:${linkMatch[1]}`);
        if (activeTriggers.value.some(t => plainText.includes(t))) {
          lastAutoPilotTime.value = now;
          setTimeout(() => { invoke('write_pty', { tabId: id, data: "\r" }); }, 300);
        }
      }
    }
    terminalManager.write(id, bytes);
  });

  const ports: any = await invoke('get_active_ports');
  if (ports.agent) currentAgentPort.value = ports.agent;
  setTimeout(() => { 
    refreshExplorer(); 
    invoke('load_remote_skills').then((s: any) => skills.value = s); 
    nextTick(() => { initCharts(); setInterval(fetchStats, 3000); });
  }, 1000);
};

const refreshExplorer = async () => { if (isConnected.value) realFiles.value = await invoke('ls_remote', { path: currentPath.value }); };
const changeDir = (path: string) => {
  if (path === '..') {
    const parts = currentPath.value.split('/').filter(p => p);
    parts.pop(); currentPath.value = '/' + parts.join('/');
  } else {
    currentPath.value = (currentPath.value === '/' ? '' : currentPath.value) + '/' + path;
  }
  const saved = localStorage.getItem('ter_fast_access');
  let list = saved ? JSON.parse(saved) : [];
  list = [currentPath.value, ...list.filter((p: string) => p !== currentPath.value)].slice(0, 5);
  localStorage.setItem('ter_fast_access', JSON.stringify(list));
  refreshExplorer();
};

const onFastAccess = async (path: string) => {
  currentPath.value = path;
  if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${path}"\r` });
  refreshExplorer();
};

const refreshWebview = async (forcedUrl?: string) => {
  if (forcedUrl) previewUrl.value = forcedUrl;
  let urlStr = previewUrl.value.trim();
  if (!urlStr) return;
  if (/^\d+$/.test(urlStr)) { urlStr = `http://localhost:${urlStr}`; previewUrl.value = urlStr; }
  const match = urlStr.match(/(?:localhost|127\.0\.0\.1):(\d+)/);
  if (match && match[1]) {
    isWebviewLoading.value = true;
    try {
      const localPort = await invoke<number>('open_dynamic_tunnel', { remotePort: parseInt(match[1]) });
      previewUrl.value = `http://localhost:${localPort}`;
    } catch (e) { console.error(e); } finally { isWebviewLoading.value = false; }
  }
};

const onMorseDown = () => { morseDownTime.value = Date.now(); if (morseTimer.value) clearTimeout(morseTimer.value); };
const onMorseUp = () => {
  const duration = Date.now() - morseDownTime.value;
  morseSequence.value += (duration < 250) ? '.' : '-';
  morseTimer.value = setTimeout(async () => {
    const char = morseMap[morseSequence.value];
    if (char && activeTabId.value) {
      morseText.value += char;
      await invoke('write_pty', { tabId: activeTabId.value, data: char });
    }
    morseSequence.value = '';
    setTimeout(() => { morseText.value = ''; }, 2000);
  }, 1000);
};

const onMorseMacro = (e: MouseEvent) => { menuX.value = e.clientX; menuY.value = e.clientY; showMorseMacro.value = true; };
const runMacro = async (cmd: string) => {
  if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: cmd + '\n' });
  showMorseMacro.value = false;
};

const renameTabAction = () => {
  const id = contextMenuTabId.value;
  if (!id) return;
  const newName = prompt("Enter new tab name:");
  if (newName) { const tab = terminalTabs.value.find(t => t.id === id); if (tab) tab.title = newName; }
  showContextMenu.value = false;
};

const copyTabIdAction = async () => { if (contextMenuTabId.value) await navigator.clipboard.writeText(contextMenuTabId.value); showContextMenu.value = false; };
const diagnoseSelection = async () => {
  const id = contextMenuTabId.value || activeTabId.value;
  if (!id) return;
  const selection = terminalManager.getSelection(id);
  const prompt = `帮我诊断以下报错并给方案：\n\n\`\`\`\n${selection}\n\`\`\``;
  if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~${prompt}\x1b[201~\r` });
  showContextMenu.value = false;
};

const addTrigger = () => { if (newTriggerStr.value && !activeTriggers.value.includes(newTriggerStr.value)) { activeTriggers.value.push(newTriggerStr.value); newTriggerStr.value = ''; } };
const removeTrigger = (t: string) => { activeTriggers.value = activeTriggers.value.filter(item => item !== t); };

const captureAndUpload = async () => { await invoke('ai_audit_ui'); };
const runSkill = async (skill: any) => { backendLogs.value.push(`[SKILL] Executing ${skill.name}...`); };

// Charts Logic
const cpuChartRef = ref<HTMLElement | null>(null), memChartRef = ref<HTMLElement | null>(null);
let cpuChart: any, memChart: any;
const cpuHistory = ref<number[]>([]), memHistory = ref<number[]>([]);
const currentCpuUsage = computed(() => cpuHistory.value.length > 0 ? cpuHistory.value[cpuHistory.value.length - 1] : 0);
const initCharts = () => { if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); if (memChartRef.value) memChart = echarts.init(memChartRef.value); };
const fetchStats = async () => {
  if (!currentAgentPort.value) return;
  try {
    const r = await fetch(`http://localhost:${currentAgentPort.value}/stats`, { headers: { 'X-Ter-Token': agentToken.value } });
    const d = await r.json();
    cpuHistory.value.push(d.cpu_usage); memHistory.value.push((d.mem_used / d.mem_total) * 100);
    if (cpuHistory.value.length > 30) { cpuHistory.value.shift(); memHistory.value.shift(); }
    cpuChart?.setOption(getChartOpt(cpuHistory.value, '#6366f1')); memChart?.setOption(getChartOpt(memHistory.value, '#a855f7'));
  } catch (e) {}
};
const getChartOpt = (d: any[], c: string) => ({ grid: { top: 5, bottom: 0, left: 0, right: 0 }, xAxis: { type: 'category', show: false }, yAxis: { type: 'value', min: 0, max: 100, show: false }, series: [{ data: d, type: 'line', smooth: true, areaStyle: { color: c }, itemStyle: { color: c }, showSymbol: false }], animation: false });

const masterPasswordStr = ref('');
const setMasterPass = async () => { await invoke('set_master_password', { password: masterPasswordStr.value }); isMasterPasswordSet.value = true; loadServers(); };
const loadServers = async () => { savedServers.value = await invoke('list_server_configs'); };

let unlistenLog: any, unlistenPty: any;
onMounted(async () => {
  window.addEventListener('contextmenu', (e) => e.preventDefault());
  const savedTriggers = localStorage.getItem('ter_active_triggers');
  if (savedTriggers) try { activeTriggers.value = JSON.parse(savedTriggers); } catch(e){}
  unlistenLog = await listen<string>('backend-log', (e) => { backendLogs.value.push(e.payload); if (backendLogs.value.length > 500) backendLogs.value.shift(); });
  window.addEventListener('keydown', (e) => { if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; });
});
onUnmounted(() => { if (unlistenLog) unlistenLog(); if (unlistenPty) unlistenPty(); });
</script>

<template>
  <div class="app-shell" @click="showContextMenu = false; showMorseMacro = false">
    <div v-if="!isMasterPasswordSet" class="modal-overlay">
      <div class="auth-card">
        <h2>🔒 Unlock Vault</h2>
        <input v-model="masterPasswordStr" type="password" placeholder="Password..." @keyup.enter="setMasterPass" />
        <button @click="setMasterPass" class="btn-primary">Unlock</button>
      </div>
    </div>

    <div v-else-if="!isConnected" class="workspace-setup">
      <div class="vault-container" :class="{ 'connecting': isConnecting }">
        <header><h3>Server Vault</h3><button @click="savedServers.push({ id: 'dummy', label: 'Local Host', user: 'root', host: '127.0.0.1' })" class="btn-add">+</button></header>
        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <div class="icon-box">SSH</div>
            <div class="info"><b>{{ s.label }}</b><br/><small>{{ s.user }}@{{ s.host }}</small></div>
          </div>
        </div>
      </div>
    </div>

    <div v-else class="main-view">
      <div v-if="showTriggerConfig" class="modal-overlay trigger-modal">
        <div class="auth-card config-card">
          <header class="modal-header"><h3>🤖 Auto-Pilot Triggers</h3><button @click="showTriggerConfig = false" class="close-btn">×</button></header>
          <div class="trigger-list-scroll">
            <div v-for="t in activeTriggers" :key="t" class="trigger-item"><span>{{ t }}</span><button @click="removeTrigger(t)" class="remove-btn">🗑️</button></div>
          </div>
          <div class="add-trigger-box"><input v-model="newTriggerStr" @keyup.enter="addTrigger" /><button @click="addTrigger" class="btn-primary mini">Add</button></div>
        </div>
      </div>

      <SettingsPanel :isOpen="showSettings" @close="showSettings = false" @update-macros="(m) => activeMacros = m" />

      <SidebarPanel 
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :cpuChartRef="(el: any) => cpuChartRef = el" :memChartRef="(el: any) => memChartRef = el"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @morse-down="onMorseDown" @morse-up="onMorseUp" @morse-context="onMorseMacro"
      />

      <main class="workspace" ref="workspaceRef" @click="activeTabId && terminalManager.focus(activeTabId)">
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename Tab</div>
          <div class="menu-item" @click="copyTabIdAction">🆔 Copy Tab ID</div>
          <div class="menu-item" @click="sendToBackground">🚀 Background Task</div>
          <div class="menu-divider"></div>
          <div class="menu-item" :class="{ disabled: !contextMenuTabId || !terminalManager.hasSelection(contextMenuTabId) }" @click="copySelectedText">📋 Copy</div>
          <div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div>
          <div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Force Close</div>
        </div>

        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
          <div class="menu-divider"></div>
          <div class="menu-item" @click="showSettings = true">⚙️ Manage Macros...</div>
        </div>

        <div v-if="morseSequence || morseText" class="morse-preview-overlay">
          <div class="sequence">{{ morseSequence }}</div>
          <div class="text">{{ morseText }}</div>
        </div>

        <nav class="tool-bar">
          <div class="status-chip"><span class="pulse purple"></span> {{ host }}</div>
          <div class="actions"><button @click="isLocked = true" class="btn-tool">Lock System</button></div>
        </nav>

        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" @terminal-context="onTerminalContextMenu" />
          </section>

          <section class="cyber-pane" v-if="cyberMode !== 0">
            <div class="cyber-container">
              <div class="cyber-logs-view">
                <header><span class="title">Cyber Logs</span></header>
                <div class="logs-container">
                  <div v-for="(log, i) in backendLogs" :key="i" class="log-line"><span class="line-num">{{ i + 1 }}</span> {{ log }}</div>
                </div>
              </div>
              <div class="cyber-divider"></div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <div class="address-input-wrapper"><span class="secure-icon">🔒</span><input v-model="previewUrl" @keyup.enter="refreshWebview()" @focus="($event.target as HTMLInputElement).select()" class="address-bar-input" /></div>
                  <button class="refresh-btn" @click="refreshWebview()" :class="{ spinning: isWebviewLoading }">{{ isWebviewLoading ? '⏳' : '⚡' }}</button>
                </nav>
                <CyberWebview :url="previewUrl" />
              </div>
            </div>
          </section>
        </div>

        <footer class="status-bar">
          <div class="status-left"><span class="item">🟢 {{ host }}</span><span class="item separator">|</span><span class="item">Agent: {{ currentAgentPort ? 'Active' : 'Offline' }}</span></div>
          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload">📸 Audit UI</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">{{ cyberMode === 1 ? '🖥️ Terminal Focus' : '🌐 Cyber View' }}</button>
            <div class="status-toggle"><span>Auto-Pilot</span><label class="mini-switch"><input type="checkbox" v-model="isAutoPilot" /><span class="slider"></span></label></div>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
.app-shell { height: 100vh; background: #09090b; color: #d4d4d8; font-family: 'Inter', system-ui; overflow: hidden; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #09090b; overflow: hidden; min-width: 0; }
.tool-bar { height: 36px; background: #09090b; border-bottom: 1px solid #27272a; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }
.status-bar { height: 24px; background: #18181b; border-top: 1px solid #27272a; color: #71717a; display: flex; justify-content: space-between; align-items: center; padding: 0 10px; font-size: 11px; z-index: 100; flex-shrink: 0; }
.status-left, .status-right { display: flex; align-items: center; gap: 15px; }
.status-btn { background: transparent; border: none; color: #a1a1aa; cursor: pointer; font-size: 11px; padding: 2px 6px; border-radius: 4px; transition: all 0.2s; }
.status-btn:hover { background: rgba(255, 255, 255, 0.08); color: #fff; }
.status-toggle { display: flex; align-items: center; gap: 8px; font-size: 11px; color: #71717a; }
.mini-switch { position: relative; display: inline-block; width: 24px; height: 12px; }
.mini-switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #3f3f46; transition: .4s; border-radius: 12px; }
.slider:before { position: absolute; content: ""; height: 8px; width: 8px; left: 2px; bottom: 2px; background-color: white; transition: .4s; border-radius: 50%; }
input:checked + .slider { background-color: #3b82f6; }
input:checked + .slider:before { transform: translateX(12px); }
.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; position: relative; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; background: #09090b; overflow: hidden; display: flex; flex-direction: column; }
.cyber-container { display: flex; flex-direction: column; height: 100%; flex: 1; overflow: hidden; }
.cyber-logs-view { flex: 0 0 30%; display: column; background: #09090b; border-bottom: 1px solid #27272a; overflow: hidden; }
.cyber-logs-view header { padding: 8px 12px; border-bottom: 1px solid #27272a; }
.cyber-logs-view .title { font-size: 10px; color: #3b82f6; font-weight: bold; text-transform: uppercase; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; }
.log-line { margin-bottom: 2px; white-space: pre-wrap; word-break: break-all; }
.line-num { color: #3f3f46; margin-right: 8px; }
.cyber-divider { height: 1px; background: #27272a; }
.cyber-webview-wrapper { flex: 1; background: #09090b; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { height: 32px; border-bottom: 1px solid #27272a; display: flex; align-items: center; padding: 0 8px; gap: 8px; }
.address-input-wrapper { flex: 1; background: #18181b; border: 1px solid #27272a; border-radius: 6px; display: flex; align-items: center; padding: 0 8px; height: 24px; animation: breathing-border 3s infinite; }
@keyframes breathing-border { 0% { border-color: #27272a; } 50% { border-color: #3b82f6; } 100% { border-color: #27272a; } }
.address-bar-input { background: transparent; border: none; color: #a1a1aa; font-size: 10px; width: 100%; outline: none; }
.refresh-btn { background: transparent; border: none; color: #3b82f6; cursor: pointer; }
.refresh-btn.spinning { animation: spinning 1s linear infinite; }
@keyframes spinning { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
.context-menu { position: fixed; z-index: 100000; background: #18181b; border: 1px solid #3f3f46; border-radius: 6px; padding: 4px; min-width: 150px; }
.menu-header { padding: 6px 12px; font-size: 9px; color: #52525b; border-bottom: 1px solid #27272a; }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; }
.menu-item:hover { background: #3b82f6; color: #fff; }
.menu-item.danger { color: #ef4444; }
.morse-preview-overlay { position: absolute; bottom: 80px; left: 280px; background: rgba(0, 0, 0, 0.8); border: 1px solid #22c55e; padding: 10px 20px; border-radius: 8px; z-index: 1000; display: flex; flex-direction: column; align-items: center; }
.morse-preview-overlay .sequence { font-size: 24px; color: #22c55e; letter-spacing: 4px; }
.modal-overlay { position: fixed; inset: 0; background: rgba(9, 9, 11, 0.9); display: flex; align-items: center; justify-content: center; z-index: 10000; backdrop-filter: blur(4px); }
.auth-card { background: #18181b; padding: 30px; border-radius: 12px; border: 1px solid #27272a; width: 320px; }
.btn-primary { width: 100%; padding: 12px; background: #3b82f6; border: none; color: #fff; border-radius: 6px; cursor: pointer; }
.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; }
.vault-container { width: 480px; background: #18181b; border: 1px solid #27272a; border-radius: 12px; padding: 30px; }
.server-list { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.server-card { background: #09090b; border: 1px solid #27272a; padding: 15px; border-radius: 10px; display: flex; align-items: center; cursor: pointer; gap: 12px; }
</style>