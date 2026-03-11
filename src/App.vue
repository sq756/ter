<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as echarts from 'echarts';

// Import Manager and Sub-components
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import CyberWebview from './components/CyberWebview.vue';

// ==========================================
// --- GLOBAL STATE ---
// ==========================================
const isConnected = ref(false);
const isConnecting = ref(false); 
const isMasterPasswordSet = ref(false);
const isAutoPilot = ref(false); 
const isLocked = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);
const backendLogs = ref<string[]>([]);
const savedServers = ref<any[]>([]);
const host = ref('Remote Server');
const currentPath = ref('/');

// Context Menu State
const showContextMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const contextMenuTabId = ref<string | null>(null);

// Tabs State
const terminalTabs = ref<any[]>([]);
const activeTabId = ref<string | null>(null);
const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

// SFTP / Data State
const realFiles = ref<any[]>([]);
const skills = ref<any[]>([]);

// ==========================================
// --- TAB MANAGEMENT ---
// ==========================================
const createNewTab = (title = "Shell") => {
  const id = 'tab-' + Math.random().toString(36).substr(2, 9);
  terminalManager.setOnDataCallback(id, (data) => {
    if (isConnected.value) invoke('write_pty', { data });
  });
  terminalManager.getOrCreate(id);
  terminalTabs.value.push({ id, title, isBackground: false });
  activeTabId.value = id;
  return id;
};

const closeTab = (id: string) => {
  const index = terminalTabs.value.findIndex(t => t.id === id);
  if (index !== -1) {
    terminalTabs.value.splice(index, 1);
    terminalManager.remove(id);
    if (activeTabId.value === id) {
      activeTabId.value = terminalTabs.value[0]?.id || null;
    }
  }
};

const sendToBackground = () => {
  const targetId = contextMenuTabId.value || activeTabId.value;
  if (!targetId) return;
  const tab = terminalTabs.value.find(t => t.id === targetId);
  if (tab) {
    const selection = terminalManager.getSelection(tab.id).trim();
    tab.isBackground = true;
    // Semantic Naming: Use selection as process name if available
    tab.title = selection 
      ? `Proc: ${selection.substring(0, 30)}${selection.length > 30 ? '...' : ''}` 
      : `Task: ${tab.id.substr(0, 5)}`;
    
    if (activeTabId.value === targetId) {
      activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null;
      if (!activeTabId.value) createNewTab("New Shell");
    }
  }
  showContextMenu.value = false;
};

const bringToForeground = (id: string) => {
  const tab = terminalTabs.value.find(t => t.id === id);
  if (tab) {
    tab.isBackground = false;
    activeTabId.value = id;
  }
};

const onTerminalContextMenu = (payload: { e: MouseEvent, id: string }) => {
  contextMenuTabId.value = payload.id;
  menuX.value = payload.e.clientX;
  menuY.value = payload.e.clientY;
  showContextMenu.value = true;
};

// Handle right-click from sidebar processes
const onProcContext = (payload: { event: MouseEvent, tab: any }) => {
  onTerminalContextMenu({ e: payload.event, id: payload.tab.id });
};

// ==========================================
// --- CORE LOGIC: SSH ---
// ==========================================
const connectWithId = async (id: string) => { 
  if (isConnecting.value) return; 
  isConnecting.value = true;
  const s = savedServers.value.find(s => s.id === id);
  if (s) host.value = s.label || s.host;
  
  invoke('connect_with_id', { id }).then(async () => {
    isConnecting.value = false;
    await onConnected();
    backendLogs.value.push('[INFO] 已建立新会话。');
  }).catch(e => {
    isConnecting.value = false;
    alert("Connection Failed: " + e);
  });
};

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  createNewTab("Main Shell");

  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<number[]>('pty-data', (event) => {
    const data = new Uint8Array(event.payload);
    const text = new TextDecoder().decode(data);

    // [反向控制]: 拦截来自 AI 的 [TER_RPC] 指令
    if (text.includes('[TER_RPC]')) {
      try {
        const rpcMatch = text.match(/\[TER_RPC\]\s*({.*})/);
        if (rpcMatch && rpcMatch[1]) {
          const rpc = JSON.parse(rpcMatch[1]);
          console.log("[RPC] Intercepted from AI:", rpc);
          if (rpc.action === 'screenshot') {
            // AI 请求截图，悄悄截图上传，不显示在屏幕上
            captureAndUpload(true);
            return; // 拦截成功，不再广播到终端显示
          }
        }
      } catch (e) {
        console.warn("RPC Parse Error:", e);
      }
    }

    terminalManager.broadcast(data);
  });

  const ports: any = await invoke('get_active_ports');
  if (ports.agent) currentAgentPort.value = ports.agent;

  setTimeout(() => {
    refreshExplorer();
    invoke('load_remote_skills').then((s: any) => skills.value = s);
    terminalManager.fitAll();
  }, 1000);
};

const refreshExplorer = async () => {
  if (isConnected.value) realFiles.value = await invoke('ls_remote', { path: currentPath.value });
};

const changeDir = (path: string) => {
  if (path === '..') {
    const parts = currentPath.value.split('/').filter(p => p);
    parts.pop();
    currentPath.value = '/' + parts.join('/');
  } else {
    currentPath.value = (currentPath.value === '/' ? '' : currentPath.value) + '/' + path;
  }
  refreshExplorer();
};

// ==========================================
// --- CORE LOGIC: Visual Audit Loop ---
// ==========================================
const workspaceRef = ref<HTMLElement | null>(null);
const cyberPaneRef = ref<HTMLElement | null>(null);

const captureAndUpload = async (auto = false) => {
  const target = (cyberPaneRef.value && cyberPaneRef.value.offsetParent !== null)
      ? cyberPaneRef.value
      : workspaceRef.value;
    
  if (!target) return;
  
  isAutoPilot.value = true;
  try {
    const html2canvas = (await import('html2canvas')).default;
    const canvas = await html2canvas(target, { 
      backgroundColor: '#000000', 
      useCORS: true, 
      scale: 2.0,
      logging: false,
      allowTaint: true,
      ignoreElements: (element) => element.classList.contains('terminal-pane') && target !== workspaceRef.value
    });
    
    const base64Data = canvas.toDataURL('image/png');
    const remotePath = await invoke<string>('upload_ui_snapshot', { base64Data });
    const lastLogs = backendLogs.value.slice(-10).join('\n');
    await invoke('write_remote_text', { text: lastLogs, remotePath: '/tmp/current_logs.json' });

    const msg = auto 
      ? `[SYSTEM] Audit Done: ${remotePath} + Logs Sync` 
      : `Manual audit completed. Snapshot: ${remotePath}, Logs: /tmp/current_logs.json`;
    
    await invoke('write_pty', { data: msg + "\n" });
  } catch (e) { 
    console.error("Capture Failed:", e); 
    backendLogs.value.push(`[ERROR] Visual Audit failed: ${e}`);
  }
};

const runSkill = async (skill: any) => {
  if (!isConnected.value) return;

  // 1. Vision-Loop: Capture & Upload if required by Skill
  if (skill.context_requirement?.require_screenshot) {
    backendLogs.value.push(`[SYSTEM] Skill "${skill.name}" requires UI context. Synchronizing...`);
    try {
      await captureAndUpload(true);
    } catch (e) {
      console.error("Vision-Loop sync failed:", e);
    }
  }

  // 2. Execute RPC
  const rpc = skill.rpc || skill.trigger;
  if (rpc) {
    if (rpc.includes('audit') || rpc.toLowerCase().includes('gemini') || rpc.includes('ter')) {
      isAutoPilot.value = true;
    }
    invoke('write_pty', { data: rpc.endsWith('\n') ? rpc : rpc + "\r\n" });
  }
};

// ==========================================
// --- DATA FLOW & UTILS ---
// ==========================================
watch(isConnected, (val) => {
  if (val) {
    nextTick(() => {
      initCharts();
      setInterval(fetchStats, 3000);
    });
  }
});

const cpuChartRef = ref<HTMLElement | null>(null);
const memChartRef = ref<HTMLElement | null>(null);
let cpuChart: any, memChart: any;
const cpuHistory = ref<number[]>([]), memHistory = ref<number[]>([]);
const currentCpuUsage = computed(() => cpuHistory.value.length > 0 ? cpuHistory.value[cpuHistory.value.length - 1] : 0);

const initCharts = () => { 
  if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); 
  if (memChartRef.value) memChart = echarts.init(memChartRef.value); 
};

const fetchStats = async () => {
  if (!currentAgentPort.value) return;
  try {
    const r = await fetch(`http://localhost:${currentAgentPort.value}/stats`, { headers: { 'X-Ter-Token': agentToken.value } });
    const d = await r.json();
    cpuHistory.value.push(d.cpu_usage); memHistory.value.push((d.mem_used / d.mem_total) * 100);
    if (cpuHistory.value.length > 30) { cpuHistory.value.shift(); memHistory.value.shift(); }
    cpuChart?.setOption(getChartOpt(cpuHistory.value, '#6366f1'));
    memChart?.setOption(getChartOpt(memHistory.value, '#a855f7'));
  } catch (e) {}
};
const getChartOpt = (d: any[], c: string) => ({ grid: { top: 5, bottom: 0, left: 0, right: 0 }, xAxis: { type: 'category', show: false }, yAxis: { type: 'value', min: 0, max: 100, show: false }, series: [{ data: d, type: 'line', smooth: true, areaStyle: { color: c }, itemStyle: { color: c }, showSymbol: false }], animation: false });

const masterPasswordStr = ref('');
const setMasterPass = async () => { await invoke('set_master_password', { password: masterPasswordStr.value }); isMasterPasswordSet.value = true; loadServers(); };
const loadServers = async () => { savedServers.value = await invoke('list_server_configs'); };

let unlistenLog: any, unlistenPty: any;
onMounted(async () => {
  unlistenLog = await listen<string>('backend-log', (e) => { 
    backendLogs.value.push(e.payload); 
    if (backendLogs.value.length > 100) backendLogs.value.shift(); 
  });
  window.addEventListener('keydown', (e) => { 
    if (e.altKey && e.key.toLowerCase() === 'l') {
      isLocked.value = !isLocked.value;
      if (!isLocked.value) nextTick(() => { initCharts(); terminalManager.fitAll(); });
    }
  });
  window.addEventListener('mouseup', () => { terminalManager.fitAll(); });
});
onUnmounted(() => { if (unlistenLog) unlistenLog(); if (unlistenPty) unlistenPty(); });
</script>

<template>
  <div class="app-shell" @click="showContextMenu = false">
    <div v-if="!isMasterPasswordSet" class="modal-overlay">
      <div class="auth-card">
        <h2>🔒 Unlock Vault</h2>
        <input v-model="masterPasswordStr" type="password" placeholder="Password..." @keyup.enter="setMasterPass" />
        <button @click="setMasterPass" class="btn-primary">Unlock</button>
      </div>
    </div>

    <div v-else-if="!isConnected" class="workspace-setup">
      <div class="vault-container" :class="{ 'connecting': isConnecting }">
        <header>
          <h3>Server Vault</h3>
          <button @click="savedServers.push({ id: 'dummy', label: 'Local Host', user: 'root', host: '127.0.0.1' })" class="btn-add">+</button>
        </header>
        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <div class="icon-box">SSH</div>
            <div class="info"><b>{{ s.label }}</b><br/><small>{{ s.user }}@{{ s.host }}</small></div>
          </div>
        </div>
      </div>
    </div>

    <div v-else class="main-view">
      <SidebarPanel 
        :files="realFiles" 
        :bgTabs="backgroundTabs"
        :skills="skills"
        :cpuChartRef="(el: any) => cpuChartRef = el"
        :memChartRef="(el: any) => memChartRef = el"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground"
        @switch-mode="(mode: number) => cyberMode = mode"
        @proc-context="onProcContext"
        @run-skill="runSkill"
        @change-dir="changeDir"
      />

      <main class="workspace" ref="workspaceRef" @click="activeTabId && terminalManager.focus(activeTabId)">
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <div class="menu-item" @click="sendToBackground">🚀 Background Task</div>
          <div class="menu-divider"></div>
          <div class="menu-item disabled">📋 Copy (Coming)</div>
          <div class="menu-item disabled">📥 Paste (Coming)</div>
        </div>

        <nav class="tool-bar">
          <div class="status-chip"><span class="pulse purple"></span> {{ host }}</div>
          <div class="actions">
            <button @click="isLocked = true" class="btn-tool">Lock</button>
            <button @click="cyberMode = cyberMode === 1 ? 0 : 1" class="btn-tool">
              {{ cyberMode === 1 ? 'Focus' : 'Cyber View' }}
            </button>
          </div>
        </nav>

        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs 
              :tabs="terminalTabs" 
              :activeTabId="activeTabId"
              @switch-tab="bringToForeground"
              @close-tab="closeTab"
              @new-tab="createNewTab()"
              @terminal-context="onTerminalContextMenu"
            />
          </section>

          <section class="cyber-pane" v-if="cyberMode !== 0" ref="cyberPaneRef">
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
                <CyberWebview ref="webviewRef" :url="`http://localhost:${currentAgentPort || 5173}`" />
              </div>
            </div>
          </section>
        </div>
      </main>
    </div>

    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
.app-shell { height: 100vh; background: #050505; color: #e4e4e7; font-family: 'Inter', system-ui; overflow: hidden; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; min-width: 0; }
.tool-bar { height: 45px; background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }
.workspace-body { flex: 1; display: flex; overflow: hidden; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; }
.cyber-pane { width: 380px; height: 100%; border-left: 1px solid #1a1a1c; background: #000; overflow: hidden; }
.cyber-container { display: flex; flex-direction: column; height: 100%; }
.cyber-logs-view { flex: 0 0 40%; display: flex; flex-direction: column; background: #0a0a0a; border-bottom: 1px solid #1a1a1c; overflow: hidden; }
.cyber-logs-view header { padding: 8px 12px; background: #0c0c0e; border-bottom: 1px solid #1a1a1c; }
.cyber-logs-view .title { font-size: 10px; color: #6366f1; font-weight: bold; text-transform: uppercase; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; }
.log-line { margin-bottom: 2px; white-space: pre-wrap; word-break: break-all; }
.line-num { color: #3f3f46; margin-right: 8px; }
.cyber-divider { height: 1px; background: #1a1a1c; }
.cyber-webview-wrapper { flex: 1; background: #000; position: relative; }
.context-menu { position: fixed; z-index: 100000; background: #18181b; border: 1px solid #3f3f46; border-radius: 6px; padding: 4px; min-width: 150px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; border-radius: 4px; }
.menu-item:hover { background: #3f3f46; color: #fff; }
.menu-item.disabled { color: #52525b; cursor: not-allowed; }
.menu-divider { height: 1px; background: #27272a; margin: 4px 0; }
.status-chip { font-size: 11px; color: #a1a1aa; display: flex; align-items: center; }
.pulse { width: 8px; height: 8px; background: #d946ef; border-radius: 50%; margin-right: 8px; box-shadow: 0 0 10px #d946ef; }
.btn-tool { background: transparent; border: 1px solid #27272a; color: #a1a1aa; padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 11px; margin-left: 10px; }
.btn-tool:hover { border-color: #6366f1; color: #fff; }
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 10000; }
.auth-card { background: #111; padding: 30px; border-radius: 12px; border: 1px solid #333; width: 320px; }
.auth-card input { width: 100%; padding: 12px; background: #000; border: 1px solid #333; color: #fff; border-radius: 6px; margin-bottom: 15px; }
.btn-primary { width: 100%; padding: 12px; background: #6366f1; border: none; color: #fff; border-radius: 6px; cursor: pointer; font-weight: bold; }
.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; background: #000; }
.vault-container { width: 450px; background: #111; border: 1px solid #333; border-radius: 12px; padding: 25px; }
.server-card { background: #1a1a1a; border: 1px solid #333; padding: 12px; border-radius: 8px; display: flex; align-items: center; cursor: pointer; margin-bottom: 10px; }
</style>
