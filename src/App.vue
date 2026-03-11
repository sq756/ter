<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import '@xterm/xterm/css/xterm.css';
import * as echarts from 'echarts';
import * as webllm from '@mlc-ai/web-llm';

// ==========================================
// --- MODULE: State Management ---
// ==========================================
const isConnected = ref(false);
const isMasterPasswordSet = ref(false);
const isProcessing = ref(false);
const agentToken = ref('');
const backendLogs = ref<string[]>([]);
const savedServers = ref<any[]>([]);
const showAddServer = ref(false);
const host = ref('Remote Server');

// Layout Control
const sidebarWidth = ref(260);
const isResizing = ref(false);
const showDashboard = ref(true);
const showAiPanel = ref(false);
const cyberMode = ref(0); // 0: Normal, 1: Full, 2: Split-H, 3: Split-V

const toggleCyber = () => {
  cyberMode.value = (cyberMode.value + 1) % 4;
  setTimeout(() => {
    if (fitAddon) fitAddon.fit();
    cpuChart?.resize();
    memChart?.resize();
  }, 50);
};

// Sidebar Resizing
const startResizing = () => {
  isResizing.value = true;
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResizing);
};

const handleMouseMove = (e: MouseEvent) => {
  if (isResizing.value) {
    sidebarWidth.value = Math.max(150, Math.min(600, e.clientX));
    cpuChart?.resize();
    memChart?.resize();
  }
};

const stopResizing = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResizing);
};

// Widgets Data
const stats = ref<any>(null);
const managedTasks = ref<any[]>([]);
const showAddTask = ref(false);

const mockFiles = ref([
  { name: 'src', is_dir: true, size: 0 },
  { name: 'package.json', is_dir: false, size: 1240 },
  { name: 'README.md', is_dir: false, size: 5600 },
]);

const mockProcesses = ref([
  { pid: 1024, name: 'nginx', cpu_usage: 1.2, mem_usage: 0.5 },
  { pid: 2048, name: 'node', cpu_usage: 15.4, mem_usage: 4.2 },
  { pid: 4096, name: 'python3', cpu_usage: 0.5, mem_usage: 1.1 },
]);

// Server Management
const newServer = ref({ label: '', host: '', user: '', pass: '', port: 22 });
const addServer = async () => {
  const id = Date.now().toString();
  await invoke('save_server_config', { 
    config: { id, label: newServer.value.label, host: newServer.value.host, user: newServer.value.user, port: newServer.value.port, password_enc: newServer.value.pass, key_path: null }
  });
  showAddServer.value = false;
  loadServers();
  newServer.value = { label: '', host: '', user: '', pass: '', port: 22 };
};

// ==========================================
// --- MODULE: Terminal & Context Menu ---
// ==========================================
const terminalRef = ref<HTMLElement | null>(null);
const fontSize = ref(14);
const showContextMenu = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const selectedText = ref('');

let term: Terminal;
let fitAddon: FitAddon;

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
  } catch (e) { console.error('Paste failed', e); }
  showContextMenu.value = false;
};

const onTerminalContextMenu = (e: MouseEvent) => {
  e.preventDefault();
  selectedText.value = term.getSelection();
  menuPos.value = { x: e.clientX, y: e.clientY };
  showContextMenu.value = true;
};

const handleWheel = (e: WheelEvent) => {
  if (e.ctrlKey) {
    e.preventDefault();
    const newSize = Math.min(Math.max(fontSize.value + (e.deltaY > 0 ? -1 : 1), 8), 40);
    if (newSize !== fontSize.value) {
      fontSize.value = newSize;
      term.options.fontSize = fontSize.value;
      nextTick(() => fitAddon.fit());
    }
  }
};

// ==========================================
// --- MODULE: Agent API (The "Failed to Fetch" Fixer) ---
// ==========================================
const agentFetch = async (endpoint: string, options: any = {}) => {
  if (!agentToken.value) throw new Error('Agent token missing');
  const url = `http://localhost:54321${endpoint}`;
  const headers = { 
    'X-Ter-Token': agentToken.value, 
    'Content-Type': 'application/json',
    ...options.headers 
  };
  
  try {
    const res = await fetch(url, { ...options, headers });
    if (!res.ok) throw new Error(`Agent error: ${res.status}`);
    return res;
  } catch (e) {
    console.warn(`Fetch to ${endpoint} failed. Agent might be restarting...`);
    throw e;
  }
};

const runAsTask = async () => {
  const text = selectedText.value || term.getSelection();
  if (!text) return;
  const parts = text.trim().split(/\s+/);
  try {
    await agentFetch('/task/start', {
      method: 'POST',
      body: JSON.stringify({ id: 'task-' + Date.now(), command: parts[0], args: parts.slice(1) })
    });
    showContextMenu.value = false;
    fetchTasks();
  } catch (e) { alert('Failed to start task. Is Agent running?'); }
};

// ==========================================
// --- MODULE: AI Sidekick ---
// ==========================================
const MODEL_ID = "SmolLM2-135M-Instruct-v0.1-q4f16_1-MLC";
const aiEngine = ref<any>(null);
const aiLoading = ref(false);
const aiProgress = ref('');
const aiChatHistory = ref<{ role: string, content: string }[]>([]);
const userMessage = ref('');
const isAiInitialized = ref(false);
const chatRef = ref<HTMLElement | null>(null);

const initAi = async () => {
  if (isAiInitialized.value) return;
  aiLoading.value = true;
  try {
    const appConfig = { model_list: [{
      model_id: MODEL_ID, model: MODEL_ID,
      model_lib: `${webllm.modelLibURLPrefix}SmolLM2-135M-Instruct-v0.1-q4f16_1-MLC-webgpu.wasm`,
      model_url: `ter-model://localhost/`,
      low_resource_required: true,
    }]};
    aiEngine.value = await webllm.CreateMLCEngine(MODEL_ID, { 
      appConfig, initProgressCallback: (p) => { aiProgress.value = `Loading: ${Math.round(p.progress * 100)}%`; } 
    });
    isAiInitialized.value = true;
    aiProgress.value = 'AI Ready';
  } catch (e) { aiProgress.value = 'Error: ' + e; } finally { aiLoading.value = false; }
};

const sendToAi = async () => {
  if (!userMessage.value || !aiEngine.value) return;
  aiChatHistory.value.push({ role: 'user', content: userMessage.value });
  userMessage.value = ''; 
  aiLoading.value = true;
  const assistantIdx = aiChatHistory.value.push({ role: 'assistant', content: '' }) - 1;
  try {
    const chunks = await aiEngine.value.chat.completions.create({ messages: aiChatHistory.value.slice(0, -1) as any, stream: true });
    let reply = "";
    for await (const chunk of chunks) {
      reply += chunk.choices[0]?.delta?.content || "";
      if (aiChatHistory.value[assistantIdx]) {
        aiChatHistory.value[assistantIdx].content = reply;
      }
      nextTick(() => chatRef.value && (chatRef.value.scrollTop = chatRef.value.scrollHeight));
    }
  } catch (e) { 
    if (aiChatHistory.value[assistantIdx]) {
      aiChatHistory.value[assistantIdx].content = 'Error: ' + e; 
    }
  } finally { aiLoading.value = false; }
};

// ==========================================
// --- MODULE: Charts & Stats ---
// ==========================================
const cpuChartRef = ref<HTMLElement | null>(null);
const memChartRef = ref<HTMLElement | null>(null);
let cpuChart: echarts.ECharts | null = null;
let memChart: echarts.ECharts | null = null;
const cpuHistory = ref<number[]>([]);
const memHistory = ref<number[]>([]);

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

const getChartOpt = (_label: string, data: any[], color: string) => ({
  grid: { top: 10, bottom: 0, left: 0, right: 0 },
  xAxis: { type: 'category', show: false },
  yAxis: { type: 'value', min: 0, max: 100, show: false },
  series: [{ data, type: 'line', smooth: true, areaStyle: { color }, itemStyle: { color }, showSymbol: false }],
  animation: false
});

// ==========================================
// --- CORE: Lifecycle & Connection ---
// ==========================================
const fetchTasks = async () => { try { const res = await agentFetch('/task/list'); managedTasks.value = await res.json(); } catch(e){} };
const fetchStats = async () => { try { const res = await agentFetch('/stats'); stats.value = await res.json(); updateCharts(stats.value); } catch(e){} };

let unlistenLog: (() => void) | null = null;
let unlistenPty: (() => void) | null = null;

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');

  // Clean up existing listener to prevent "sticky keys" due to multiple triggers
  if (unlistenPty) { unlistenPty(); unlistenPty = null; }
  unlistenPty = await listen<number[]>('pty-data', (event) => {
    term.write(new Uint8Array(event.payload));
  });

  await nextTick();
  if (terminalRef.value) {
    term.open(terminalRef.value);
    try { term.loadAddon(new WebglAddon()); } catch (e) { }
    setTimeout(() => { fitAddon.fit(); term.focus(); }, 150);
  }
  initCharts();
  setInterval(() => { fetchStats(); fetchTasks(); }, 2000);
};

onMounted(async () => {
  // Listen for backend logs
  unlistenLog = await listen<string>('backend-log', (event) => {
    backendLogs.value.push(event.payload);
    if (backendLogs.value.length > 100) backendLogs.value.shift();
  });

  term = new Terminal({
    cursorBlink: true, fontSize: fontSize.value,
    fontFamily: "'JetBrains Mono', monospace",
    theme: { background: '#000', foreground: '#fafafa' },
    allowTransparency: true
  });
  fitAddon = new FitAddon(); term.loadAddon(fitAddon);
  term.onData(data => {
    if (isConnected.value && !isProcessing.value) {
      invoke('write_pty', { data });
    }
  });
  window.addEventListener('resize', () => { fitAddon.fit(); cpuChart?.resize(); memChart?.resize(); });
});

onUnmounted(() => {
  if (unlistenLog) unlistenLog();
  if (unlistenPty) unlistenPty();
});


// Helper: Master Pass & Connect
const masterPasswordStr = ref('');
const setMasterPass = async () => {
  await invoke('set_master_password', { password: masterPasswordStr.value });
  isMasterPasswordSet.value = true;
  loadServers();
};
const loadServers = async () => { savedServers.value = await invoke('list_server_configs'); };
const connectWithId = async (id: string) => { 
  const s = savedServers.value.find(s => s.id === id);
  if (s) host.value = s.label || s.host;
  await invoke('connect_with_id', { id }); 
  onConnected(); 
};
const deleteServer = async (id: string) => { await invoke('delete_server_config', { id }); loadServers(); };
</script>

<template>
  <div class="app-shell" :class="{ 'cyber': cyberMode }" @contextmenu.prevent @click="showContextMenu = false">
    
    <!-- Cyber Background Layer (Digital Rain Effect) -->
    <div v-if="cyberMode > 0" class="cyber-logs-layer">
      <div v-for="(log, i) in backendLogs" :key="i" class="cyber-log-line">{{ log }}</div>
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
      <div class="vault-container">
        <header>
          <h3>Server Vault</h3>
          <button @click="showAddServer = true" class="btn-ghost">+</button>
        </header>
        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-item" @click="connectWithId(s.id)">
            <div class="info"><b>{{ s.label }}</b><br/><small>{{ s.user }}@{{ s.host }}</small></div>
            <button @click.stop="deleteServer(s.id)" class="btn-del">✕</button>
          </div>
        </div>
      </div>

      <!-- Add Server Modal -->
      <div v-if="showAddServer" class="modal-overlay">
        <div class="auth-card add-server">
          <h2>Add Remote Server</h2>
          <input v-model="newServer.label" placeholder="Label (e.g. My Server)" />
          <input v-model="newServer.host" placeholder="Host (IP or Domain)" />
          <input v-model="newServer.user" placeholder="Username" />
          <input v-model="newServer.pass" type="password" placeholder="Password" />
          <input v-model.number="newServer.port" type="number" placeholder="Port (Default 22)" />
          <div class="modal-btns">
            <button @click="showAddServer = false" class="btn-ghost">Cancel</button>
            <button @click="addServer" class="btn-primary">Save Server</button>
          </div>
        </div>
      </div>
    </div>

    <!-- Phase 3: Main UI (MODULAR) -->
    <div v-else class="main-view">
      
      <!-- Modular Sidebar -->
      <aside class="side-bar" :style="{ width: sidebarWidth + 'px' }" v-if="showDashboard && cyberMode !== 1">
        <div class="module widget-resources">
          <header>System Health</header>
          <div class="chart-container">
            <div ref="cpuChartRef" class="mini-chart"></div>
            <div ref="memChartRef" class="mini-chart"></div>
          </div>
        </div>

        <div class="module widget-processes">
          <header>Top Processes</header>
          <ul class="data-list">
            <li v-for="p in (stats?.processes || mockProcesses)" :key="p.pid">
              <span class="name">{{ p.name }}</span>
              <span class="val">{{ Math.round(p.cpu_usage) }}%</span>
            </li>
          </ul>
        </div>

        <div class="module widget-files">
          <header>Remote Files</header>
          <ul class="data-list">
            <li v-for="f in mockFiles" :key="f.name">
              <span class="icon">{{ f.is_dir ? '📁' : '📄' }}</span>
              <span class="name">{{ f.name }}</span>
            </li>
          </ul>
        </div>

        <div class="module widget-tasks">
          <header>Managed Tasks <button @click="showAddTask = !showAddTask">+</button></header>
          <ul class="task-items">
            <li v-for="t in managedTasks" :key="t.id">
              <span class="name">{{ t.command }}</span>
              <span class="status" :class="t.status">{{ t.status }}</span>
            </li>
          </ul>
        </div>

        <div class="sidebar-footer">
          <header>Task Monitor</header>
          <div class="monitor-output">
            <div v-for="(log, i) in backendLogs.slice(-3)" :key="i" class="monitor-line">{{ log }}</div>
          </div>
        </div>
      </aside>

      <!-- Sidebar Handle -->
      <div class="resizer-handle" @mousedown="startResizing" v-if="showDashboard"></div>

      <!-- Center Workspace -->
      <main class="workspace" :class="'cyber-layout-' + cyberMode">
        <nav class="tool-bar">
          <div class="active-session">🟢 {{ host }}</div>
          <div class="actions">
            <button @click="toggleCyber" :class="{ active: cyberMode > 0 }">
              Cyber <span v-if="cyberMode > 0">[{{ cyberMode === 1 ? 'Full' : cyberMode === 2 ? 'Split-H' : 'Split-V' }}]</span>
            </button>
            <button @click="showAiPanel = !showAiPanel">AI Sidekick</button>
          </div>
        </nav>

        <div class="workspace-body">
          <div class="terminal-view" v-show="cyberMode !== 1">
            <div class="terminal-inner" ref="terminalRef" @wheel="handleWheel" @contextmenu="onTerminalContextMenu"></div>
            
            <!-- MODULAR CONTEXT MENU -->
            <div v-if="showContextMenu" class="floating-menu" :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }">
              <button @click="handleCopy">📋 Copy Selection</button>
              <button @click="handlePaste">📥 Paste to Terminal</button>
              <hr/>
              <button @click="runAsTask" class="special">🚀 Run as Background Task</button>
            </div>
          </div>

          <div v-if="cyberMode > 1" class="cyber-logs-panel">
            <div v-for="(log, i) in backendLogs" :key="i" class="cyber-log-line">{{ log }}</div>
          </div>
        </div>
      </main>

      <!-- AI Sidekick Panel -->
      <Transition name="slide">
        <div v-if="showAiPanel && cyberMode !== 1" class="ai-drawer">
          <header>AI Sidekick <button @click="showAiPanel = false">✕</button></header>
          <div v-if="!isAiInitialized" class="ai-setup">
            <p>On-device LLM (WebGPU)</p>
            <button @click="initAi">Initialize Core</button>
            <small>{{ aiProgress }}</small>
          </div>
          <div v-else class="ai-chat">
            <div class="messages" ref="chatRef">
              <div v-for="(m, i) in aiChatHistory" :key="i" :class="['bubble', m.role]">{{ m.content }}</div>
            </div>
            <input v-model="userMessage" @keyup.enter="sendToAi" placeholder="Ask AI..." />
          </div>
        </div>
      </Transition>

    </div>
  </div>
</template>

<style scoped>
/* ==========================================
   Modular Layout Styles
   ========================================== */
.app-shell { height: 100vh; background: #09090b; color: #eee; font-family: 'Inter', sans-serif; overflow: hidden; position: relative; transition: background 0.5s ease; }
.app-shell.cyber { background: transparent; }

/* Cyber Mode Glass Effects */
.cyber .side-bar, 
.cyber .tool-bar, 
.cyber .module, 
.cyber .terminal-inner,
.cyber .ai-drawer {
  background: rgba(18, 18, 21, 0.7) !important;
  backdrop-filter: blur(12px) saturate(180%);
  border-color: rgba(63, 63, 70, 0.4) !important;
}

.cyber-logs-layer {
  position: absolute;
  inset: 0;
  z-index: -1;
  padding: 20px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  background: #050505;
}

.cyber-log-line {
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: rgba(34, 197, 94, 0.3);
  line-height: 1.4;
  white-space: pre-wrap;
}

/* Dashboard/Sidebar */
.main-view { display: flex; height: 100%; width: 100%; }
.side-bar { background: #121215; border-right: 1px solid #27272a; display: flex; flex-direction: column; overflow-y: auto; flex-shrink: 0; }
.module { padding: 15px; border-bottom: 1px solid #27272a; }
.module header { font-size: 11px; text-transform: uppercase; color: #71717a; margin-bottom: 10px; display: flex; justify-content: space-between; }

/* Charts */
.mini-chart { height: 60px; margin-bottom: 10px; border-radius: 4px; overflow: hidden; }

/* Workspace */
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; }
.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.tool-bar { height: 45px; background: #121215; border-bottom: 1px solid #27272a; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }

/* Cyber Layouts */
.cyber-layout-2 .workspace-body { flex-direction: row; }
.cyber-layout-3 .workspace-body { flex-direction: column; }

.cyber-logs-panel {
  flex: 0 0 20%;
  background: #1a1a1a;
  padding: 15px;
  overflow-y: auto;
  border-left: 1px solid #333;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: rgba(34, 197, 94, 0.8);
  z-index: 10;
}

.terminal-view { flex: 0 0 80%; padding: 20px; position: relative; }

.cyber-layout-3 .cyber-logs-panel {
  flex: 0 0 25%;
  border-left: none;
  border-top: 1px solid #333;
}

.terminal-inner { height: 100%; background: #000; border-radius: 8px; border: 1px solid #27272a; padding: 10px; }

/* Data Lists (Processes/Files) */
.data-list { list-style: none; padding: 0; margin: 0; font-size: 12px; }
.data-list li { display: flex; justify-content: space-between; padding: 4px 0; color: #a1a1aa; border-bottom: 1px solid rgba(255,255,255,0.05); }
.data-list .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 120px; }
.data-list .val { color: #6366f1; font-weight: bold; }
.data-list .icon { margin-right: 8px; font-size: 10px; }

/* Task Monitor Sidebar Footer */
.sidebar-footer { margin-top: auto; padding: 15px; background: #09090b; border-top: 1px solid #27272a; }
.sidebar-footer header { font-size: 10px; text-transform: uppercase; color: #71717a; margin-bottom: 8px; display: block; }
.monitor-output { background: #000; padding: 8px; border-radius: 4px; border: 1px solid #27272a; height: 60px; overflow: hidden; display: flex; flex-direction: column; gap: 2px; }
.monitor-line { font-family: 'JetBrains Mono', monospace; font-size: 9px; color: #22c55e; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; opacity: 0.8; }

/* Modal Helpers */
.modal-btns { display: flex; gap: 10px; justify-content: flex-end; margin-top: 10px; }
.add-server input { margin-bottom: 12px; }

/* Floating Context Menu (Elegant) */
.floating-menu { position: fixed; background: #1c1c1f; border: 1px solid #3f3f46; border-radius: 8px; padding: 5px; z-index: 9999; box-shadow: 0 10px 25px rgba(0,0,0,0.5); min-width: 180px; }
.floating-menu button { width: 100%; padding: 10px 15px; text-align: left; background: transparent; border: none; color: #eee; font-size: 13px; cursor: pointer; border-radius: 4px; }
.floating-menu button:hover { background: #6366f1; }
.floating-menu .special { color: #818cf8; font-weight: bold; }
.floating-menu hr { border: 0; border-top: 1px solid #3f3f46; margin: 5px 0; }

/* Resizer Handle */
.resizer-handle { width: 4px; cursor: col-resize; transition: background 0.2s; }
.resizer-handle:hover { background: #6366f1; }

/* AI Drawer */
.ai-drawer { width: 350px; background: #121215; border-left: 1px solid #27272a; display: flex; flex-direction: column; }
.ai-chat { flex: 1; display: flex; flex-direction: column; padding: 15px; overflow: hidden; }
.messages { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; margin-bottom: 10px; }
.bubble { padding: 10px; border-radius: 8px; font-size: 13px; max-width: 85%; }
.bubble.user { align-self: flex-end; background: #6366f1; }
.bubble.assistant { align-self: flex-start; background: #27272a; }

/* Utils */
.modal-overlay { position: fixed; inset: 0; background: #09090b; display: flex; align-items: center; justify-content: center; z-index: 10000; }
.auth-card { background: #18181b; padding: 40px; border-radius: 20px; border: 1px solid #27272a; width: 350px; text-align: center; }
input { background: #000; border: 1px solid #27272a; color: #fff; padding: 12px; border-radius: 8px; width: 100%; margin-bottom: 20px; }
.btn-primary { background: #6366f1; color: #fff; padding: 12px; border-radius: 8px; width: 100%; border: none; cursor: pointer; font-weight: bold; }
.btn-ghost { background: transparent; color: #71717a; border: none; cursor: pointer; }
.task-items { list-style: none; padding: 0; font-size: 12px; }
.task-items li { display: flex; justify-content: space-between; padding: 5px 0; }
.status.running { color: #22c55e; }
</style>
