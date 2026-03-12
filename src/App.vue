<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as echarts from 'echarts';
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import CyberWebview from './components/CyberWebview.vue';
import SettingsPanel from './components/SettingsPanel.vue';

const isConnected = ref(false), isConnecting = ref(false), isMasterPasswordSet = ref(false);
const isAutoPilot = ref(false), lastAutoPilotTime = ref(0), connectionStatus = ref<'connected' | 'busy' | 'disconnected'>('disconnected');
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const showSettings = ref(false), activeMacros = ref<{name: string, cmd: string}[]>([]);
const morseSequence = ref(''), morseText = ref(''), showMorseMacro = ref(false), morseTimer = ref<any>(null), isMorsePressed = ref(false);
const morseMap: Record<string, string> = { '.-': 'A', '-...': 'B', '-.-.': 'C', '-..': 'D', '.': 'E', '..-.': 'F', '--.': 'G', '....': 'H', '..': 'I', '.---': 'J', '-.-': 'K', '.-..': 'L', '--': 'M', '-.': 'N', '---': 'O', '.--.': 'P', '--.-': 'Q', '.-.': 'R', '...': 'S', '-': 'T', '..-': 'U', '...-': 'V', '.--': 'W', '-..-': 'X', '-.--': 'Y', '--..': 'Z', '-----': '0', '.----': '1', '..---': '2', '...--': '3', '....-': '4', '.....': '5', '-....': '6', '--...': '7', '---..': '8', '----.': '9' };
const isLocked = ref(false), cyberMode = ref(0), agentToken = ref(''), currentAgentPort = ref<number | null>(null), previewUrl = ref('http://localhost:5173'), isWebviewLoading = ref(false);
const backendLogs = ref<string[]>([]), savedServers = ref<any[]>([]), host = ref('Remote Server'), currentPath = ref('/'), realFiles = ref<any[]>([]), skills = ref<any[]>([]);
const showContextMenu = ref(false), menuX = ref(0), menuY = ref(0), contextMenuTabId = ref<string | null>(null), hasErrorSelection = ref(false);
const terminalTabs = ref<any[]>([]), activeTabId = ref<string | null>(null);
const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground)), storageKey = computed(() => `ter_tabs_${host.value.replace(/\s+/g, '_')}`);

let statsIntervalId: any = null;
const showAddServerForm = ref(false);
const newServer = ref({ label: '', host: '', port: 22, user: 'root', password_enc: '' });

watch(terminalTabs, (val) => { if (isConnected.value) localStorage.setItem(storageKey.value, JSON.stringify(val)); }, { deep: true });
watch(activeTriggers, (val) => { localStorage.setItem('ter_active_triggers', JSON.stringify(val)); }, { deep: true });

const calculateMenuPosition = (e: MouseEvent, estimatedHeight = 250, estimatedWidth = 160) => {
  let x = e.clientX, y = e.clientY;
  if (y + estimatedHeight > window.innerHeight) y = window.innerHeight - estimatedHeight - 10;
  if (x + estimatedWidth > window.innerWidth) x = window.innerWidth - estimatedWidth - 10;
  menuX.value = x; menuY.value = y;
};

const createNewTab = async (title = "Shell", skipPty = false, existingId?: string) => {
  const id = existingId || 'tab-' + Math.random().toString(36).substr(2, 9);
  terminalManager.setOnDataCallback(id, (tid, data) => { if (!skipPty && isConnected.value) invoke('write_pty', { tabId: tid, data }); });
  terminalManager.getOrCreate(id);
  if (!skipPty && isConnected.value) {
    try {
      await invoke('spawn_new_pty', { tabId: id });
      setTimeout(() => invoke('write_pty', { tabId: id, data: "\r" }), 500);
    } catch (e) { backendLogs.value.push(`[ERROR] PTY Fail: ${e}`); }
  }
  if (!existingId) terminalTabs.value.push({ id, title, isBackground: false });
  activeTabId.value = id; return id;
};

const viewHistory = async (originalTabId: string) => {
  const t = terminalTabs.value.find(x => x.id === originalTabId);
  const playbackId = await createNewTab(`Playback: ${t?.title || originalTabId}`, true);
  try {
    const logs = await invoke<number[][]>('get_terminal_logs', { tabId: originalTabId, limit: 1000 });
    for (const chunk of logs) { terminalManager.write(playbackId, new Uint8Array(chunk)); await new Promise(r => setTimeout(r, 20)); }
  } catch (e) { terminalManager.write(playbackId, `\r\n[ERROR] History Fail: ${e}\r\n`); }
};

const closeTab = (id: string) => { const idx = terminalTabs.value.findIndex(t => t.id === id); if (idx !== -1) { terminalTabs.value.splice(idx, 1); terminalManager.remove(id); if (activeTabId.value === id) activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null; } };
const copySelectedText = async () => { const id = contextMenuTabId.value || activeTabId.value; if (id) { const s = terminalManager.getSelection(id); if (s) await navigator.clipboard.writeText(s); } showContextMenu.value = false; };
const pasteFromClipboard = async () => { const id = contextMenuTabId.value || activeTabId.value; if (id) { try { const t = await navigator.clipboard.readText(); if (t) invoke('write_pty', { tabId: id, data: t }); } catch(e){} } showContextMenu.value = false; };
const sendToBackground = () => { const tid = contextMenuTabId.value || activeTabId.value; if (tid) { const tab = terminalTabs.value.find(t => t.id === tid); if (tab) { const s = terminalManager.getSelection(tab.id).trim(); tab.isBackground = true; tab.title = s ? `Proc: ${s.substring(0, 20)}...` : `Task: ${tab.id.substr(0, 5)}`; if (activeTabId.value === tid) activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null; } } showContextMenu.value = false; };
const bringToForeground = (id: string) => { const t = terminalTabs.value.find(t => t.id === id); if (t) { t.isBackground = false; activeTabId.value = id; } };
const onTerminalContextMenu = (p: { e: MouseEvent, id: string }) => { contextMenuTabId.value = p.id; calculateMenuPosition(p.e); const s = terminalManager.getSelection(p.id); hasErrorSelection.value = s.toLowerCase().includes('error') || s.toLowerCase().includes('exception') || s.includes('\x1b[31m'); showContextMenu.value = true; };

const connectWithId = async (id: string) => { if (isConnecting.value) return; isConnecting.value = true; connectionStatus.value = 'busy'; const s = savedServers.value.find(s => s.id === id); if (s) host.value = s.label || s.host; invoke('connect_with_id', { id }).then(async () => { isConnecting.value = false; await onConnected(); }).catch(e => { isConnecting.value = false; connectionStatus.value = 'disconnected'; alert("Fail: " + e); }); };

const onConnected = async () => {
  isConnected.value = true; connectionStatus.value = 'connected';
  try { agentToken.value = await invoke('get_agent_token'); } catch(e){}
  const saved = localStorage.getItem(storageKey.value);
  if (saved) {
    try {
      const ts = JSON.parse(saved); terminalTabs.value = ts;
      for (const t of ts) await createNewTab(t.title, false, t.id);
      activeTabId.value = ts.find((t: any) => !t.isBackground)?.id || ts[0]?.id;
    } catch (e) { await createNewTab("Main Shell", false, "tab-1"); }
  } else if (terminalTabs.value.length === 0) { await createNewTab("Main Shell", false, "tab-1"); }
  if (statsIntervalId) clearInterval(statsIntervalId);
  setTimeout(() => {
    refreshExplorer(); invoke('load_remote_skills').then((s: any) => skills.value = s).catch(()=>{});
    nextTick(() => { initCharts(); statsIntervalId = setInterval(fetchStats, 3000); });
  }, 1000);
};

const refreshExplorer = async () => { if (isConnected.value) realFiles.value = await invoke('ls_remote', { path: currentPath.value }); };
const changeDir = (p: string) => {
  if (p === '..') { const pts = currentPath.value.split('/').filter(x => x); pts.pop(); currentPath.value = '/' + pts.join('/'); } else { currentPath.value = (currentPath.value === '/' ? '' : currentPath.value) + '/' + p; }
  const s = localStorage.getItem('ter_fast_access'); let l = s ? JSON.parse(s) : []; l = [currentPath.value, ...l.filter((x: string) => x !== currentPath.value)].slice(0, 5); localStorage.setItem('ter_fast_access', JSON.stringify(l)); refreshExplorer();
};
const onFastAccess = async (p: string) => { currentPath.value = p; if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${p}"\r` }); refreshExplorer(); };
const refreshWebview = async (fUrl?: string) => {
  if (fUrl) previewUrl.value = fUrl; let u = previewUrl.value.trim(); if (!u) return; if (/^\d+$/.test(u)) { u = `http://localhost:${u}`; previewUrl.value = u; }
  const m = u.match(/(?:localhost|127\.0\.0\.1):(\d+)/); if (m && m[1]) { isWebviewLoading.value = true; try { const p = await invoke<number>('open_dynamic_tunnel', { remotePort: parseInt(m[1]) }); previewUrl.value = `http://localhost:${p}`; } catch (e) {} finally { isWebviewLoading.value = false; } }
};
const handleExtractDOM = async () => { backendLogs.value.push(`[INFO] Extracting DOM...`); await invoke('extract_cyber_dom'); };
const onDomExtracted = async (md: string) => { if (activeTabId.value) { await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~${md}\x1b[201~\r` }); backendLogs.value.push(`[INFO] Snapshot injected.`); } };

const possibleLetters = computed(() => {
  if (!morseSequence.value) return "";
  const candidates = Object.entries(morseMap).filter(([code]) => code.startsWith(morseSequence.value)).slice(0, 5).map(([code, char]) => `${char}(${code})`);
  return candidates.length ? "Next: " + candidates.join(" ") : "";
});
const handleMorseMouse = (e: MouseEvent) => {
  if (e.button === 1) { onMorseMacro(e); return; }
  isMorsePressed.value = true; setTimeout(() => { isMorsePressed.value = false; }, 100);
  if (e.button === 0) morseSequence.value += '.'; else if (e.button === 2) morseSequence.value += '-';
  if (morseTimer.value) clearTimeout(morseTimer.value); morseTimer.value = setTimeout(commitMorse, 800);
};
const handleMorseWheel = (e: WheelEvent) => {
  if (activeTabId.value) { if (e.deltaY < 0) invoke('write_pty', { tabId: activeTabId.value, data: "\r" }); else invoke('write_pty', { tabId: activeTabId.value, data: "\x7f" }); }
};
const commitMorse = async () => {
  const char = morseMap[morseSequence.value]; if (char && activeTabId.value) { morseText.value += char; await invoke('write_pty', { tabId: activeTabId.value, data: char }); }
  morseSequence.value = ''; setTimeout(() => { if (!morseSequence.value) morseText.value = ''; }, 2000);
};

const onMorseMacro = (e: MouseEvent) => { calculateMenuPosition(e, 200); showMorseMacro.value = true; };
const runMacro = async (c: string) => { if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: c + '\n' }); showMorseMacro.value = false; };
const renameTabAction = () => { const id = contextMenuTabId.value; if (id) { const n = prompt("New name:"); if (n) { const t = terminalTabs.value.find(x => x.id === id); if (t) t.title = n; } } showContextMenu.value = false; };
const copyTabIdAction = async () => { if (contextMenuTabId.value) await navigator.clipboard.writeText(contextMenuTabId.value); showContextMenu.value = false; };
const diagnoseSelection = async () => { const id = contextMenuTabId.value || activeTabId.value; if (id) { const s = terminalManager.getSelection(id); if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~帮我诊断并给方案：\n\n\`\`\`\n${s}\n\`\`\`\x1b[201~\r` }); } showContextMenu.value = false; };
const captureAndUpload = async () => { await invoke('ai_audit_ui'); };
const runSkill = async (s: any) => { backendLogs.value.push(`[SKILL] Exec: ${s.name}`); };

const saveNewServer = async () => {
  if (!newServer.value.host || !newServer.value.user) return;
  await invoke('save_server_config', { config: { id: 'node-' + Math.random().toString(36).substr(2, 9), ...newServer.value } });
  showAddServerForm.value = false; loadServers();
};

const cpuChartRef = ref<HTMLElement | null>(null), memChartRef = ref<HTMLElement | null>(null);
let cpuChart: any, memChart: any; const cpuHistory = ref<number[]>([]), memHistory = ref<number[]>([]);
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
const preventDefaultContextMenu = (e: MouseEvent) => e.preventDefault();
const handleGlobalKeyDown = (e: KeyboardEvent) => { if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; };

onMounted(async () => {
  window.addEventListener('contextmenu', preventDefaultContextMenu);
  window.addEventListener('keydown', handleGlobalKeyDown);
  const st = localStorage.getItem('ter_active_triggers'); if (st) try { activeTriggers.value = JSON.parse(st); } catch(e){}
  const sm = localStorage.getItem('ter_macros'); if (sm) try { activeMacros.value = JSON.parse(sm); } catch(e){}
  unlistenLog = await listen<string>('backend-log', (e) => { backendLogs.value.push(e.payload); if (backendLogs.value.length > 500) backendLogs.value.shift(); });
  
  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<any>('pty-data', (ev) => {
    const { id, data } = ev.payload; const bytes = new Uint8Array(data);
    if (terminalManager) terminalManager.write(id, bytes);
    if (connectionStatus.value === 'connected') { connectionStatus.value = 'busy'; setTimeout(() => { if (isConnected.value) connectionStatus.value = 'connected'; }, 200); }
    if (isAutoPilot.value && id === activeTabId.value) {
      const text = new TextDecoder().decode(bytes);
      const pt = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
      const actionMatch = pt.match(/\[TER_ACTION:\s*(click|type)\((\d+)(?:,\s*"(.*?)")?\)\]/);
      if (actionMatch) {
        const action = actionMatch[1], eid = actionMatch[2], txt = actionMatch[3] || "";
        const code = action === 'click' ? `window.TerAgent.click(${eid})` : `window.TerAgent.type(${eid}, ${JSON.stringify(txt)})`;
        invoke('eval_cyber_webview', { code });
      } else if (!pt.includes('tab-') && (Date.now() - lastAutoPilotTime.value) > 500) {
        const lm = pt.match(/http:\/\/localhost:(\d+)/); if (lm && lm[1]) refreshWebview(`http://localhost:${lm[1]}`);
        if (activeTriggers.value.some(t => pt.includes(t))) { lastAutoPilotTime.value = Date.now(); setTimeout(() => { invoke('write_pty', { tabId: id, data: "\r" }); }, 300); }
      }
    }
  });
});

onUnmounted(() => {
  window.removeEventListener('contextmenu', preventDefaultContextMenu);
  window.removeEventListener('keydown', handleGlobalKeyDown);
  if (unlistenLog) unlistenLog(); if (unlistenPty) unlistenPty();
  if (statsIntervalId) clearInterval(statsIntervalId); if (morseTimer.value) clearTimeout(morseTimer.value);
});
</script>

<template>
  <div class="app-shell" @click="showContextMenu = false; showMorseMacro = false">
    <div v-if="!isMasterPasswordSet" class="modal-overlay">
      <div class="auth-card cyber-card">
        <h2 class="cyber-title">SYSTEM OVERRIDE</h2>
        <div class="cyber-subtitle">/// AUTHENTICATION_REQUIRED</div>
        <input v-model="masterPasswordStr" type="password" placeholder="ENTER ACCESS KEY..." @keyup.enter="setMasterPass" class="cyber-input" />
        <button @click="setMasterPass" class="btn-primary">INITIALIZE</button>
      </div>
    </div>

    <div v-else-if="!isConnected" class="workspace-setup">
      <div class="vault-container cyber-card" :class="{ 'connecting': isConnecting }">
        <header>
          <h2 class="cyber-title">AUTHORIZED NODES</h2>
          <button @click="showAddServerForm = true" class="btn-add">+</button>
        </header>
        
        <div v-if="showAddServerForm" class="add-server-overlay">
          <div class="cyber-form">
            <input v-model="newServer.label" placeholder="LABEL" class="cyber-input" />
            <div class="row">
              <input v-model="newServer.host" placeholder="HOST" class="cyber-input" />
              <input v-model.number="newServer.port" placeholder="PORT" class="cyber-input small" />
            </div>
            <input v-model="newServer.user" placeholder="USER" class="cyber-input" />
            <input v-model="newServer.password_enc" type="password" placeholder="PASSWORD" class="cyber-input" />
            <div class="actions">
              <button @click="saveNewServer" class="btn-primary mini">SAVE</button>
              <button @click="showAddServerForm = false" class="btn-primary mini danger">CANCEL</button>
            </div>
          </div>
        </div>

        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <div class="icon-box">NODE</div>
            <div class="info"><b>{{ (s.label || 'UNTITLED').toUpperCase() }}</b><br/><small>{{ s.user }}@{{ s.host }}</small></div>
          </div>
          <div v-if="savedServers.length === 0" class="empty-nodes">NO AUTHORIZED NODES FOUND</div>
        </div>
      </div>
    </div>

    <div v-else class="main-view">
      <SettingsPanel :isOpen="showSettings" @close="showSettings = false" @update-macros="(m) => activeMacros = m" />
      <SidebarPanel 
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :cpuChartRef="(el: any) => cpuChartRef = el" :memChartRef="(el: any) => memChartRef = el"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
      />

      <main class="workspace" ref="workspaceRef" @click="activeTabId && terminalManager.focus(activeTabId)">
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename Tab</div><div class="menu-item" @click="copyTabIdAction">🆔 Copy ID</div><div class="menu-item" @click="sendToBackground">🚀 Background</div>
          <div class="menu-divider"></div><div class="menu-item" @click="copySelectedText">📋 Copy</div><div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div><div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Force Close</div>
        </div>
        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
          <div class="menu-divider"></div><div class="menu-item" @click="showSettings = true">⚙️ Manage...</div>
        </div>
        <div v-if="morseSequence || morseText" class="morse-preview-overlay">
          <div class="sequence">{{ morseSequence }}</div>
          <div class="text">{{ morseText }}</div>
          <div class="candidates" v-if="possibleLetters">{{ possibleLetters }}</div>
        </div>
        <nav class="tool-bar"><div class="status-chip"><span class="pulse purple"></span> {{ host }}</div><div class="actions"><button @click="isLocked = true" class="btn-tool">Lock System</button></div></nav>
        <div class="workspace-body">
          <section class="terminal-pane"><TerminalTabs :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" @terminal-context="onTerminalContextMenu" /></section>
          <section class="cyber-pane" v-if="cyberMode !== 0">
            <div class="cyber-container">
              <div class="cyber-logs-view"><header><span class="title">Cyber Logs</span></header><div class="logs-container"><div v-for="(log, i) in backendLogs" :key="i" class="log-line"><span class="line-num">{{ i + 1 }}</span> {{ log }}</div></div></div>
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
            <button class="status-btn" @click="captureAndUpload">📸 Audit</button>
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
.app-shell { height: 100vh; background: #000; color: #d4d4d8; font-family: 'JetBrains Mono', monospace; overflow: hidden; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; min-width: 0; }
.cyber-card { background: #09090b !important; border: 1px solid #22c55e !important; box-shadow: 0 0 15px rgba(34, 197, 94, 0.2) !important; border-radius: 0 !important; }
.cyber-title { color: #22c55e !important; font-family: 'JetBrains Mono', monospace; letter-spacing: 2px; text-transform: uppercase; margin-bottom: 5px; }
.cyber-subtitle { font-size: 10px; color: #166534; margin-bottom: 20px; letter-spacing: 1px; }
.cyber-input { background: #000 !important; border: 1px solid #27272a !important; color: #22c55e !important; font-family: 'JetBrains Mono', monospace !important; border-radius: 0 !important; outline: none !important; padding: 10px !important; margin-bottom: 15px; }
.cyber-input:focus { border-color: #22c55e !important; box-shadow: 0 0 5px rgba(34, 197, 94, 0.3); }
.btn-primary { background: transparent !important; border: 1px solid #22c55e !important; color: #22c55e !important; font-family: 'JetBrains Mono', monospace !important; text-transform: uppercase; letter-spacing: 1px; border-radius: 0 !important; transition: all 0.2s ease !important; cursor: pointer; padding: 12px; font-weight: bold; }
.btn-primary:hover { background: rgba(34, 197, 94, 0.2) !important; box-shadow: 0 0 10px rgba(34, 197, 94, 0.5) !important; }
.btn-primary.mini { padding: 6px 12px; font-size: 11px; }
.btn-primary.danger { border-color: #ef4444 !important; color: #ef4444 !important; }
.btn-primary.danger:hover { background: rgba(239, 68, 68, 0.2) !important; box-shadow: 0 0 10px rgba(239, 68, 68, 0.5) !important; }

.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; background: #000; }
.vault-container { width: 520px; padding: 40px; position: relative; }
.vault-container header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px; border-bottom: 1px solid #27272a; padding-bottom: 15px; }
.btn-add { background: transparent; border: 1px solid #22c55e; color: #22c55e; width: 28px; height: 28px; cursor: pointer; font-size: 20px; display: flex; align-items: center; justify-content: center; transition: all 0.2s; }
.btn-add:hover { background: #22c55e; color: #000; }

.add-server-overlay { margin-bottom: 30px; padding: 20px; background: rgba(34, 197, 94, 0.05); border: 1px dashed #22c55e; }
.cyber-form { display: flex; flex-direction: column; }
.cyber-form .row { display: flex; gap: 10px; }
.cyber-form .row .small { width: 100px; }
.cyber-form .actions { display: flex; gap: 10px; margin-top: 10px; }

.server-list { display: grid; grid-template-columns: 1fr 1fr; gap: 15px; }
.server-card { background: #050505; border: 1px solid #18181b; padding: 15px; display: flex; align-items: center; cursor: pointer; transition: all 0.2s; gap: 15px; }
.server-card:hover { border-color: #22c55e; transform: translateY(-2px); box-shadow: 0 5px 15px rgba(34, 197, 94, 0.1); }
.server-card .icon-box { background: rgba(34, 197, 94, 0.1); color: #22c55e; padding: 4px 8px; font-size: 10px; font-weight: bold; border: 1px solid rgba(34, 197, 94, 0.3); }
.server-card small { color: #52525b; font-size: 11px; }
.empty-nodes { grid-column: span 2; text-align: center; color: #3f3f46; font-size: 12px; padding: 40px; border: 1px dashed #18181b; }

.status-bar { height: 24px; background: #000; border-top: 1px solid #18181b; color: #52525b; display: flex; justify-content: space-between; align-items: center; padding: 0 10px; font-size: 10px; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; transition: all 0.1s; }
.tiny-dot.active { transform: scale(1.1); box-shadow: 0 0 8px #22c55e; filter: brightness(1.5); }
.stealth-zone { display: flex; align-items: center; gap: 8px; cursor: pointer; height: 100%; padding: 0 5px; }
.stealth-zone:hover { background: rgba(34, 197, 94, 0.05); }

.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; position: relative; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; background: #000; overflow: hidden; display: flex; flex-direction: column; }
.cyber-container { display: flex; flex-direction: column; height: 100%; flex: 1; overflow: hidden; }
.cyber-logs-view { flex: 0 0 30%; display: flex; flex-direction: column; background: #000; border-bottom: 1px solid #27272a; overflow: hidden; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; }
.cyber-webview-wrapper { flex: 1; background: #000; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { height: 32px; border-bottom: 1px solid #27272a; display: flex; align-items: center; padding: 0 8px; gap: 8px; }
.address-input-wrapper { flex: 1; background: #050505; border: 1px solid #18181b; height: 24px; animation: breathing-border 3s infinite; display: flex; align-items: center; padding: 0 8px; }
@keyframes breathing-border { 0% { border-color: #18181b; } 50% { border-color: #22c55e; } 100% { border-color: #18181b; } }
.address-bar-input { background: transparent; border: none; color: #22c55e; font-size: 10px; width: 100%; outline: none; font-family: 'JetBrains Mono', monospace; }
.extract-btn { background: rgba(168, 85, 247, 0.1); border: 1px solid #a855f7; color: #a855f7; font-size: 10px; padding: 2px 8px; cursor: pointer; font-family: 'JetBrains Mono', monospace; }
.extract-btn:hover { background: rgba(168, 85, 247, 0.2); box-shadow: 0 0 10px #a855f7; }
.context-menu { position: fixed; z-index: 1000000; background: #09090b; border: 1px solid #22c55e; padding: 4px; min-width: 160px; box-shadow: 0 10px 25px rgba(0,0,0,0.8); }
.menu-header { padding: 6px 12px; font-size: 9px; color: #166534; border-bottom: 1px solid #18181b; margin-bottom: 4px; }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-item.danger { color: #ef4444; }
.menu-item.danger:hover { background: #ef4444; color: #000; }
.morse-preview-overlay { position: absolute; bottom: 40px; left: 10px; background: rgba(0, 0, 0, 0.9); border: 1px solid #22c55e; padding: 10px 20px; z-index: 1000; display: flex; flex-direction: column; align-items: center; pointer-events: none; }
.morse-preview-overlay .sequence { font-size: 24px; color: #22c55e; letter-spacing: 4px; }
.morse-preview-overlay .candidates { font-size: 9px; color: #166534; margin-top: 5px; }
.modal-overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.9); display: flex; align-items: center; justify-content: center; z-index: 10000; backdrop-filter: blur(8px); }
.auth-card { width: 360px; padding: 40px; }
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
.menu-divider { height: 1px; background: #18181b; margin: 4px 0; }
</style>