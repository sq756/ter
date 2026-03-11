<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import '@xterm/xterm/css/xterm.css';
import * as echarts from 'echarts';
import html2canvas from 'html2canvas';

// ==========================================
// --- MODULE: State Management ---
// ==========================================
const isConnected = ref(false);
const isConnecting = ref(false); 
const isMasterPasswordSet = ref(false);
const isAutoPilot = ref(false); 
const agentToken = ref('');
const backendLogs = ref<string[]>([]);
const savedServers = ref<any[]>([]);
const showAddServer = ref(false);
const host = ref('Remote Server');

// Layout Control
const sidebarWidth = ref(260);
const cyberRatio = ref(20); 
const isResizingSidebar = ref(false);
const isResizingCyber = ref(false);
const showDashboard = ref(true);
const showAiPanel = ref(false);
const cyberMode = ref(0); 

// Context Menus
const showContextMenu = ref(false);
const showProcessMenu = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const selectedText = ref('');
const selectedProcess = ref<any>(null);

// Animations & UI Injection
const flyingTasks = ref<{id: number, x: number, y: number}[]>([]);
const pluginToasts = ref<any[]>([]);

// ==========================================
// --- MODULE: Visual Audit & RPC ---
// ==========================================
const workspaceRef = ref<HTMLElement | null>(null);

const captureAndUpload = async (autoTriggered = false) => {
  if (!workspaceRef.value) return;
  console.log("📸 Starting visual audit capture...");
  
  try {
    const canvas = await html2canvas(workspaceRef.value, {
      backgroundColor: '#000',
      logging: false,
      useCORS: true
    });
    const base64 = canvas.toDataURL('image/png');

    const remotePath = await invoke<string>('upload_ui_snapshot', { base64Data: base64 });
    
    const triggerMsg = autoTriggered 
      ? `[SYSTEM] 自动快照已就绪: ${remotePath}` 
      : `请读取 ${remotePath}，我刚才按下了审计键，你看看现在的 UI 哪里不对劲？`;
    
    await invoke('write_pty', { data: triggerMsg + "\r" });
    
    pluginToasts.value.push({
      type: 'text',
      title: 'Visual Audit',
      message: autoTriggered ? 'Auto-Snapshot Sent to Gemini' : 'Manual Snapshot Uploaded',
      timestamp: Date.now()
    });
  } catch (e) {
    console.error("Visual Audit Failed:", e);
  }
};

// ==========================================
// --- MODULE: Plugin Renderer ---
// ==========================================
const PluginText = {
  props: ['payload'],
  template: `<div class="content">{{ payload }}</div>`
};

const PluginChart = {
  props: ['payload'],
  template: `<div class="plugin-mini-chart" ref="pChartRef" style="height:80px; margin-top:5px;"></div>`,
  setup(props: any) {
    const pChartRef = ref<HTMLElement | null>(null);
    onMounted(() => {
      if (pChartRef.value) {
        const myChart = echarts.init(pChartRef.value);
        const data = Array.isArray(props.payload) ? props.payload : [10, 52, 200, 334, 390, 330, 220];
        myChart.setOption({
          grid: { top: 5, bottom: 5, left: 5, right: 5 },
          xAxis: { type: 'category', show: false },
          yAxis: { type: 'value', show: false },
          series: [{ data, type: 'bar', itemStyle: { color: '#6366f1' } }],
          animation: true
        });
      }
    });
    return { pChartRef };
  }
};

const getComponentByType = (type: string) => {
  switch(type) {
    case 'chart': return PluginChart;
    case 'text':
    default: return PluginText;
  }
};

// ==========================================
// --- MODULE: Terminal & Interceptor ---
// ==========================================
let term: Terminal;
let fitAddon: FitAddon;

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  
  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<number[]>('pty-data', (event) => {
    const data = new Uint8Array(event.payload);
    const text = new TextDecoder().decode(data);

    if (isAutoPilot.value && text.includes('[TER_RPC]')) {
      try {
        const rpcMatch = text.match(/\[TER_RPC\]\s*({.*})/);
        if (rpcMatch && rpcMatch[1]) {
          const rpc = JSON.parse(rpcMatch[1]);
          if (rpc.action === 'screenshot') {
            captureAndUpload(true);
            return; 
          }
        }
      } catch (e) {}
    }

    term.write(data);
  });

  await nextTick();
  if (terminalRef.value) {
    term.open(terminalRef.value);
    try { term.loadAddon(new WebglAddon()); } catch (e) {}
    setTimeout(() => { fitAddon.fit(); term.focus(); }, 200);
  }
  initCharts();
  setInterval(() => { fetchStats(); fetchTasks(); }, 2000);
};

const terminalRef = ref<HTMLElement | null>(null);

const handleCopy = async () => { 
  const text = term.getSelection(); 
  if (text) { 
    await navigator.clipboard.writeText(text); 
    showContextMenu.value = false; 
  } 
};

const handlePaste = async () => { 
  try { 
    const text = await navigator.clipboard.readText(); 
    if (text && isConnected.value) { 
      await invoke('write_pty', { data: text }); 
    } 
  } catch (e) {} 
  showContextMenu.value = false; 
};

const onTerminalContextMenu = (e: MouseEvent) => { 
  e.preventDefault(); 
  selectedText.value = term.getSelection(); 
  menuPos.value = { x: e.clientX, y: e.clientY }; 
  showContextMenu.value = true; 
};

const startSidebarResize = () => { isResizingSidebar.value = true; document.addEventListener('mousemove', handleGlobalMove); document.addEventListener('mouseup', stopResizing); };
const startCyberResize = () => { isResizingCyber.value = true; document.addEventListener('mousemove', handleGlobalMove); document.addEventListener('mouseup', stopResizing); };

const handleGlobalMove = (e: MouseEvent) => {
  if (isResizingSidebar.value) sidebarWidth.value = Math.max(180, Math.min(500, e.clientX));
  if (isResizingCyber.value) { 
    const containerWidth = window.innerWidth - (showDashboard.value ? sidebarWidth.value : 0); 
    const mouseOffset = window.innerWidth - e.clientX; 
    cyberRatio.value = Math.max(10, Math.min(50, (mouseOffset / containerWidth) * 100)); 
  }
  nextTick(() => { fitAddon?.fit(); cpuChart?.resize(); memChart?.resize(); });
};

const stopResizing = () => { 
  isResizingSidebar.value = false; 
  isResizingCyber.value = false; 
  document.removeEventListener('mousemove', handleGlobalMove); 
  document.removeEventListener('mouseup', stopResizing); 
};

const connectWithId = async (id: string) => { 
  if (isConnecting.value) return; 
  isConnecting.value = true;
  try {
    const s = savedServers.value.find(s => s.id === id);
    if (s) host.value = s.label || s.host;
    await invoke('connect_with_id', { id }); 
    await onConnected();
  } catch (e) { alert("Connection Failed: " + e); } finally { isConnecting.value = false; }
};

const runAsTask = async (e: MouseEvent) => {
  const text = selectedText.value || term.getSelection();
  if (!text) return;
  const id = Date.now();
  flyingTasks.value.push({ id, x: e.clientX, y: e.clientY });
  setTimeout(() => { flyingTasks.value = flyingTasks.value.filter(t => t.id !== id); }, 800);
  const parts = text.trim().split(/\s+/);
  try { 
    await agentFetch('/task/start', { method: 'POST', body: JSON.stringify({ id: 'task-' + id, command: parts[0], args: parts.slice(1) }) }); 
    showContextMenu.value = false; 
    fetchTasks(); 
  } catch (e) {}
};

const stats = ref<any>(null);
const managedTasks = ref<any[]>([]);
const mockFiles = ref([{ name: 'bin', is_dir: true }, { name: 'etc', is_dir: true }, { name: 'home', is_dir: true }]);
const mockProcesses = ref([{ pid: 1, name: 'systemd', cpu_usage: 0.1, mem_usage: 0.2 }]);

const agentFetch = async (endpoint: string, options: any = {}) => { 
  const url = `http://localhost:54321${endpoint}`; 
  return fetch(url, { ...options, headers: { 'X-Ter-Token': agentToken.value, 'Content-Type': 'application/json', ...options.headers } }); 
};

const fetchTasks = async () => { try { const res = await agentFetch('/task/list'); managedTasks.value = await res.json(); } catch(e){} };
const fetchStats = async () => { try { const res = await agentFetch('/stats'); stats.value = await res.json(); updateCharts(stats.value); } catch(e){} };

const cpuChartRef = ref<HTMLElement | null>(null);
const memChartRef = ref<HTMLElement | null>(null);
let cpuChart: any, memChart: any;
const cpuHistory = ref<number[]>([]), memHistory = ref<number[]>([]);

const initCharts = () => { 
  if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); 
  if (memChartRef.value) memChart = echarts.init(memChartRef.value); 
};

const updateCharts = (s: any) => { 
  cpuHistory.value.push(s.cpu_usage); 
  memHistory.value.push((s.mem_used / s.mem_total) * 100); 
  if (cpuHistory.value.length > 30) { cpuHistory.value.shift(); memHistory.value.shift(); } 
  cpuChart?.setOption(getChartOpt('CPU', cpuHistory.value, '#6366f1')); 
  memChart?.setOption(getChartOpt('MEM', memHistory.value, '#a855f7')); 
};

const getChartOpt = (_l: string, d: any[], c: string) => ({ 
  grid: { top: 5, bottom: 0, left: 0, right: 0 }, 
  xAxis: { type: 'category', show: false }, 
  yAxis: { type: 'value', min: 0, max: 100, show: false }, 
  series: [{ data: d, type: 'line', smooth: true, areaStyle: { color: c }, itemStyle: { color: c }, showSymbol: false }], 
  animation: false 
});

const masterPasswordStr = ref('');
const setMasterPass = async () => { await invoke('set_master_password', { password: masterPasswordStr.value }); isMasterPasswordSet.value = true; loadServers(); };
const loadServers = async () => { savedServers.value = await invoke('list_server_configs'); };
const deleteServer = async (id: string) => { await invoke('delete_server_config', { id }); loadServers(); };

const newServer = ref({ label: '', host: '', user: '', pass: '', port: 22 });
const addServer = async () => { 
  await invoke('save_server_config', { 
    config: { id: Date.now().toString(), ...newServer.value, password_enc: newServer.value.pass, key_path: null } 
  }); 
  showAddServer.value = false; 
  loadServers(); 
};

const onProcessContext = (e: MouseEvent, p: any) => {
  e.preventDefault();
  selectedProcess.value = p;
  menuPos.value = { x: e.clientX, y: e.clientY };
  showProcessMenu.value = true;
};

const killProcess = async () => {
  if (!selectedProcess.value) return;
  try {
    await agentFetch(`/proc/kill?pid=${selectedProcess.value.pid}`);
    showProcessMenu.value = false;
    fetchStats();
  } catch (e) { alert("Failed to kill process"); }
};

let unlistenLog: any, unlistenPty: any, unlistenPlugin: any;
onMounted(async () => {
  unlistenLog = await listen<string>('backend-log', (e) => { 
    backendLogs.value.push(e.payload); 
    if (backendLogs.value.length > 100) backendLogs.value.shift(); 
  });
  
  unlistenPlugin = await listen<any>('plugin-ui-event', (e) => { 
    pluginToasts.value.push(e.payload); 
    setTimeout(() => { pluginToasts.value = pluginToasts.value.filter(t => t.timestamp !== e.payload.timestamp); }, 5000); 
  });
  
  term = new Terminal({ 
    cursorBlink: true, 
    fontSize: 14, 
    fontFamily: "'JetBrains Mono', monospace", 
    theme: { background: '#000', foreground: '#fafafa' }, 
    allowTransparency: true 
  });
  fitAddon = new FitAddon(); 
  term.loadAddon(fitAddon);
  term.onData(data => { if (isConnected.value) invoke('write_pty', { data }); });
  window.addEventListener('resize', () => { fitAddon.fit(); cpuChart?.resize(); memChart?.resize(); });
});

onUnmounted(() => { 
  if (unlistenLog) unlistenLog(); 
  if (unlistenPty) unlistenPty(); 
  if (unlistenPlugin) unlistenPlugin(); 
});
</script>

<template>
  <div class="app-shell" :class="{ 'cyber': cyberMode }" @click="showContextMenu = false; showProcessMenu = false">
    
    <!-- Task Sink Animation Layer -->
    <div class="animation-layer">
      <div v-for="t in flyingTasks" :key="t.id" class="flying-node" :style="{ left: t.x+'px', top: t.y+'px' }">🚀</div>
    </div>

    <!-- Plugin UI Injection Layer (Flow B) -->
    <div class="plugin-layer">
      <TransitionGroup name="toast">
        <div v-for="t in pluginToasts" :key="t.timestamp" class="plugin-toast">
          <header>🧩 {{ t.title }}</header>
          <component :is="getComponentByType(t.type)" :payload="t.message" />
        </div>
      </TransitionGroup>
    </div>

    <!-- Phase 1: Unlock -->
    <div v-if="!isMasterPasswordSet" class="modal-overlay">
      <div class="auth-card">
        <h2>🔒 Unlock Vault</h2>
        <input v-model="masterPasswordStr" type="password" placeholder="Master Password..." @keyup.enter="setMasterPass" />
        <button @click="setMasterPass" class="btn-primary">Unlock</button>
      </div>
    </div>

    <!-- Phase 2: Server Selection -->
    <div v-else-if="!isConnected" class="workspace-setup">
      <div class="vault-container" :class="{ 'connecting': isConnecting }">
        <header>
          <h3><span class="pulse"></span> Server Vault</h3>
          <button @click="showAddServer = true" class="btn-add">+</button>
        </header>
        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <div class="icon-box">SSH</div>
            <div class="info"><div class="label">{{ s.label }}</div><div class="addr">{{ s.user }}@{{ s.host }}</div></div>
            <button @click.stop="deleteServer(s.id)" class="btn-del">✕</button>
          </div>
        </div>
        <div v-if="isConnecting" class="connecting-mask"><div class="spinner"></div><p>Establishing SSH Tunnel...</p></div>
      </div>
      <div v-if="showAddServer" class="modal-overlay">
        <div class="auth-card glass">
          <h2>New Server</h2>
          <input v-model="newServer.label" placeholder="Label" /><input v-model="newServer.host" placeholder="Host" /><input v-model="newServer.user" placeholder="User" /><input v-model="newServer.pass" type="password" placeholder="Password" />
          <div class="modal-btns"><button @click="showAddServer = false" class="btn-ghost">Cancel</button><button @click="addServer" class="btn-primary">Save</button></div>
        </div>
      </div>
    </div>

    <!-- Phase 3: Main UI -->
    <div v-else class="main-view">
      <aside class="side-bar" :style="{ width: sidebarWidth + 'px' }">
        <div class="module sys-health">
          <header>System Health</header>
          <div class="chart-box"><div ref="cpuChartRef" class="mini-chart"></div><div ref="memChartRef" class="mini-chart"></div></div>
        </div>
        <div class="module scroller processes">
          <header>Processes</header>
          <ul class="data-list">
            <li v-for="p in (stats?.processes || mockProcesses)" :key="p.pid" @contextmenu.prevent="onProcessContext($event, p)"><span class="name">{{ p.name }}</span><span class="val">{{ Math.round(p.cpu_usage) }}%</span></li>
          </ul>
        </div>
        <div class="module scroller files">
          <header>Explorer</header>
          <ul class="data-list">
            <li v-for="f in mockFiles" :key="f.name">
              <span class="icon">{{ f.is_dir ? '📁' : '📄' }}</span>
              <span class="name">{{ f.name }}</span>
            </li>
          </ul>
        </div>
        <div class="sidebar-footer">
          <header>AI Control</header>
          <div class="ai-controls">
            <button @click="captureAndUpload(false)" class="btn-audit">📸 Audit UI</button>
            <div class="toggle-box">
              <span>Auto-Pilot</span>
              <input type="checkbox" v-model="isAutoPilot" id="auto-pilot-toggle" />
              <label for="auto-pilot-toggle" class="switch"></label>
            </div>
          </div>
        </div>
      </aside>

      <div class="resizer-h" @mousedown="startSidebarResize"></div>

      <main class="workspace" ref="workspaceRef">
        <nav class="tool-bar">
          <div class="status-chip"><span class="pulse purple"></span> {{ host }}</div>
          <div class="actions">
            <button @click="cyberMode = (cyberMode + 1) % 4" class="btn-tool">Cyber</button>
            <button @click="showAiPanel = !showAiPanel" class="btn-tool">AI</button>
          </div>
        </nav>

        <div class="workspace-body">
          <section class="terminal-pane" :style="{ flex: cyberMode > 1 ? (100 - cyberRatio) : 100 + '%' }">
            <div class="terminal-container" ref="terminalRef" @contextmenu.prevent="onTerminalContextMenu"></div>
          </section>
          <div v-if="cyberMode > 1" class="resizer-v" @mousedown="startCyberResize"></div>
          <section v-if="cyberMode > 1" class="cyber-pane" :style="{ flex: cyberRatio + '%' }">
            <header>Cyber Transparency</header>
            <div class="cyber-logs"><div v-for="(log, i) in backendLogs" :key="i" class="log-line">{{ log }}</div></div>
          </section>
        </div>
      </main>

      <!-- Context Menus -->
      <div v-if="showContextMenu" class="floating-menu" :style="{ left: menuPos.x+'px', top: menuPos.y+'px' }">
        <button @click="handleCopy">📋 Copy</button><button @click="handlePaste">📥 Paste</button><hr/><button @click="runAsTask($event)" class="special">🚀 Background Task</button>
      </div>

      <div v-if="showProcessMenu" class="floating-menu" :style="{ left: menuPos.x+'px', top: menuPos.y+'px' }">
        <div class="menu-header">PID: {{ selectedProcess?.pid }}</div>
        <button @click="killProcess" class="danger">🛑 Terminate</button>
        <button>🔍 Inspect</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Base Shell & Utils */
.app-shell { height: 100vh; background: #050505; color: #e4e4e7; font-family: 'Inter', system-ui; overflow: hidden; position: relative; }
.glass { backdrop-filter: blur(10px); background: rgba(20,20,25,0.8); }

/* AI Control Styles */
.ai-controls { display: flex; flex-direction: column; gap: 10px; }
.btn-audit { background: #6366f1; border: none; color: white; padding: 8px; border-radius: 6px; cursor: pointer; font-size: 12px; font-weight: bold; }
.btn-audit:hover { background: #818cf8; }

.toggle-box { display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: #71717a; }
.switch { position: relative; display: inline-block; width: 34px; height: 18px; }
.switch::after { content: ""; position: absolute; width: 14px; height: 14px; border-radius: 50%; background-color: white; top: 2px; left: 2px; transition: 0.3s; }
input[type="checkbox"] { display: none; }
input:checked + .switch { background-color: #6366f1; border-radius: 18px; }
input:checked + .switch::after { left: 18px; }
.switch { background-color: #333; border-radius: 18px; cursor: pointer; }

/* Existing Styles ... (Side-bar, Workspace, etc) */
.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; background: radial-gradient(circle at center, #111 0%, #000 100%); }
.vault-container { width: 450px; background: #111; border: 1px solid #333; border-radius: 12px; padding: 25px; box-shadow: 0 20px 50px rgba(0,0,0,0.8); position: relative; overflow: hidden; }
.server-card { background: #1a1a1a; border: 1px solid #333; padding: 12px; border-radius: 8px; display: flex; align-items: center; cursor: pointer; transition: 0.2s; margin-bottom: 10px; }
.main-view { display: flex; height: 100%; width: 100%; }
.side-bar { background: #0a192f; display: flex; flex-direction: column; flex-shrink: 0; border-right: 1px solid #1a1a1c; }
.module { padding: 15px; border-bottom: 1px solid #1a1a1c; }
.scroller { flex: 1; overflow-y: auto; }
.sidebar-footer { padding: 15px; background: #080809; border-top: 1px solid #1a1a1c; margin-top: auto; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; }
.tool-bar { height: 45px; background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }
.workspace-body { flex: 1; display: flex; overflow: hidden; }
.terminal-pane { padding: 15px; overflow: hidden; }
.terminal-container { height: 100%; width: 100%; }
.plugin-layer { position: fixed; top: 20px; right: 20px; z-index: 11000; display: flex; flex-direction: column; gap: 10px; pointer-events: none; }
.plugin-toast { pointer-events: auto; width: 260px; background: rgba(30, 30, 35, 0.9); backdrop-filter: blur(12px); border: 1px solid rgba(99, 102, 241, 0.5); border-radius: 10px; padding: 12px; border-left: 4px solid #6366f1; }
.floating-menu { position: fixed; background: #18181b; border: 1px solid #3f3f46; border-radius: 8px; padding: 6px; z-index: 9999; min-width: 160px; }
.pulse { display: inline-block; width: 8px; height: 8px; background: #d946ef; border-radius: 50%; margin-right: 8px; box-shadow: 0 0 10px #d946ef; animation: pulse-anim 2s infinite; }
@keyframes pulse-anim { 0% { opacity: 0.4; transform: scale(0.8); } 50% { opacity: 1; transform: scale(1.1); } 100% { opacity: 0.4; transform: scale(0.8); } }
@keyframes spin { to { transform: rotate(360deg); } }
.resizer-h { width: 4px; cursor: col-resize; z-index: 100; }
.resizer-h:hover { background: #6366f1; }
.resizer-v { width: 6px; cursor: col-resize; background: #111; }
</style>
