<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import '@xterm/xterm/css/xterm.css';
import * as echarts from 'echarts';
import * as webllm from '@mlc-ai/web-llm';

const terminalRef = ref<HTMLElement | null>(null);
const cpuChartRef = ref<HTMLElement | null>(null);
const memChartRef = ref<HTMLElement | null>(null);

let cpuChart: echarts.ECharts | null = null;
let memChart: echarts.ECharts | null = null;

const cpuHistory = ref<number[]>([]);
const memHistory = ref<number[]>([]);
const timeLabels = ref<string[]>([]);
const MAX_HISTORY = 30;

const host = ref('100.72.208.74');
const user = ref('sq');
const password = ref('sq');
const label = ref('Home Server');
const masterPassword = ref('');
const isMasterPasswordSet = ref(false);
const isConnected = ref(false);
const isProcessing = ref(false); // New: Tracking connection/save state
const showDashboard = ref(true);
const errorMsg = ref('');

const savedServers = ref<any[]>([]);
const showAddServer = ref(false);

const stats = ref<any>(null);
const agentToken = ref('');
const fileList = ref<any[]>([]);
const currentPath = ref('.');

const managedTasks = ref<any[]>([]);
const showAddTask = ref(false);
const newTaskCmd = ref('');
const selectedTaskLog = ref('');
const showLogModal = ref(false);

const guiStatus = ref({ installed: false, running: false });
const showGui = ref(false);

const aiEngine = ref<any>(null);
const aiLoading = ref(false);
const aiProgress = ref('');
const aiChatHistory = ref<{ role: string, content: string }[]>([]);
const userMessage = ref('');
const isAiInitialized = ref(false);
const showAiPanel = ref(false);
const chatRef = ref<HTMLElement | null>(null);

const scrollToBottom = () => {
  setTimeout(() => {
    if (chatRef.value) {
      chatRef.value.scrollTop = chatRef.value.scrollHeight;
    }
  }, 100);
};

const MODEL_ID = "SmolLM2-135M-Instruct-v0.1-q4f16_1-MLC";

let term: Terminal;
let fitAddon: FitAddon;
let unlistenPty: (() => void) | null = null;
let statsInterval: number | null = null;

const initCharts = () => {
  if (cpuChartRef.value) {
    if (cpuChart) cpuChart.dispose();
    cpuChart = echarts.init(cpuChartRef.value);
    cpuChart.setOption(getChartOption('CPU Usage (%)', '#6366f1'));
  }
  if (memChartRef.value) {
    if (memChart) memChart.dispose();
    memChart = echarts.init(memChartRef.value);
    memChart.setOption(getChartOption('Memory Usage (%)', '#a855f7'));
  }
};

const getChartOption = (title: string, color: string) => ({
  title: { text: title, textStyle: { color: '#71717a', fontSize: 10, fontWeight: 'normal' }, left: 'center' },
  grid: { top: 25, bottom: 5, left: 30, right: 5 },
  xAxis: { type: 'category', show: false },
  yAxis: { type: 'value', min: 0, max: 100, splitLine: { lineStyle: { color: '#27272a' } } },
  series: [{ data: title.includes('CPU') ? cpuHistory.value : memHistory.value, type: 'line', smooth: true, showSymbol: false, areaStyle: { color }, itemStyle: { color } }],
  animation: false
});

const updateCharts = (stats: any) => {
  const now = new Date().toLocaleTimeString();
  const memPerc = (stats.mem_used / stats.mem_total) * 100;
  cpuHistory.value.push(stats.cpu_usage);
  memHistory.value.push(memPerc);
  timeLabels.value.push(now);
  if (cpuHistory.value.length > MAX_HISTORY) {
    cpuHistory.value.shift();
    memHistory.value.shift();
    timeLabels.value.shift();
  }
  cpuChart?.setOption({ xAxis: { data: timeLabels.value }, series: [{ data: cpuHistory.value }] });
  memChart?.setOption({ xAxis: { data: timeLabels.value }, series: [{ data: memHistory.value }] });
};

onMounted(async () => {
  term = new Terminal({
    cursorBlink: true,
    fontFamily: '"JetBrains Mono", Menlo, monospace',
    fontSize: 13,
    theme: { background: '#000', foreground: '#fafafa' }
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  
  term.onData(async (data) => { if (isConnected.value) await invoke('write_pty', { data }); });
  term.onResize(async (size) => { if (isConnected.value) await invoke('resize_pty', { cols: size.cols, rows: size.rows }); });
  window.addEventListener('resize', () => { 
    if (isConnected.value) {
      fitAddon?.fit(); 
      cpuChart?.resize(); 
      memChart?.resize(); 
    }
  });
});

onUnmounted(() => {
  if (unlistenPty) unlistenPty();
  if (statsInterval) clearInterval(statsInterval);
  term?.dispose();
  cpuChart?.dispose();
  memChart?.dispose();
});

const setMasterPass = async () => {
  if (!masterPassword.value) return;
  isProcessing.value = true;
  try {
    await invoke('set_master_password', { password: masterPassword.value });
    isMasterPasswordSet.value = true;
    loadServers();
  } catch (e) { errorMsg.value = 'Security Error: ' + e; }
  finally { isProcessing.value = false; }
};

const loadServers = async () => {
  try { savedServers.value = await invoke('list_server_configs'); } catch (e) { console.error(e); }
};

const saveServer = async () => {
  isProcessing.value = true;
  errorMsg.value = '';
  try {
    const config = { id: window.crypto.randomUUID(), label: label.value, host: host.value, user: user.value, port: 22, password_enc: password.value, key_path: null };
    await invoke('save_server_config', { config });
    showAddServer.value = false;
    loadServers();
  } catch (e) { errorMsg.value = String(e); }
  finally { isProcessing.value = false; }
};

const deleteServer = async (id: string) => {
  if (!confirm('Are you sure?')) return;
  try { await invoke('delete_server_config', { id }); loadServers(); } catch (e) { errorMsg.value = String(e); }
};

const connectWithId = async (id: string) => {
  isProcessing.value = true;
  errorMsg.value = '';
  try { await invoke('connect_with_id', { id }); await onConnected(); } catch (e) { errorMsg.value = String(e); }
  finally { isProcessing.value = false; }
};

const connect = async () => {
  isProcessing.value = true;
  errorMsg.value = '';
  try { await invoke('connect_to_ssh', { host: host.value, user: user.value, pass: password.value }); await onConnected(); } catch (e) { errorMsg.value = String(e); }
  finally { isProcessing.value = false; }
};

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  
  await nextTick();
  
  if (terminalRef.value) {
    term.open(terminalRef.value);
    try { term.loadAddon(new WebglAddon()); } catch (e) { console.warn('WebGL Addon failed', e); }
    setTimeout(() => {
      fitAddon.fit();
      invoke('resize_pty', { cols: term.cols, rows: term.rows });
      term.focus();
    }, 150);
  }
  
  fetchFiles('.');
  unlistenPty = await listen<number[]>('pty-data', (event) => { term.write(new Uint8Array(event.payload)); });
  statsInterval = window.setInterval(() => { fetchStats(); fetchTasks(); fetchGuiStatus(); }, 2000);
  initCharts();
};

const initAi = async () => {
  if (isAiInitialized.value) return;
  aiLoading.value = true;
  try {
    const engine = await webllm.CreateMLCEngine(MODEL_ID, { initProgressCallback: (p) => { aiProgress.value = `Loading: ${Math.round(p.progress * 100)}%`; } });
    aiEngine.value = engine;
    isAiInitialized.value = true;
    aiProgress.value = 'AI Online';
  } catch (e) { aiProgress.value = 'Error: ' + e; } finally { aiLoading.value = false; }
};

const sendToAi = async (msg: string = '') => {
  const content = msg || userMessage.value;
  if (!content || !aiEngine.value) return;
  aiChatHistory.value.push({ role: 'user', content });
  userMessage.value = ''; aiLoading.value = true; scrollToBottom();
  try {
    const reply = await aiEngine.value.chat.completions.create({ messages: aiChatHistory.value as any, stream: false });
    aiChatHistory.value.push({ role: 'assistant', content: reply.choices[0].message.content || '' });
    scrollToBottom();
  } catch (e) { aiChatHistory.value.push({ role: 'assistant', content: 'Error: ' + e }); } finally { aiLoading.value = false; }
};

const explainTerminalError = async () => {
  showAiPanel.value = true;
  if (!isAiInitialized.value) await initAi();
  const lines = [];
  const totalLines = term.buffer.active.length;
  for (let i = Math.max(0, totalLines - 15); i < totalLines; i++) {
    const line = term.buffer.active.getLine(i);
    if (line) lines.push(line.translateToString(true));
  }
  sendToAi(`Analyze this terminal output:\n\n${lines.join('\n')}`);
};

const explainLogs = async () => {
  showAiPanel.value = true;
  if (!isAiInitialized.value) await initAi();
  sendToAi(`Analyze these logs:\n\n${selectedTaskLog.value}`);
};

const fetchGuiStatus = async () => {
  try {
    const res = await fetch('http://localhost:54321/gui/status', { headers: { 'X-Ter-Token': agentToken.value } });
    if (res.ok) guiStatus.value = await res.json();
  } catch (e) { console.error(e); }
};

const initGui = async () => {
  try {
    await fetch('http://localhost:54321/gui/init', { headers: { 'X-Ter-Token': agentToken.value } });
    fetchGuiStatus();
  } catch (e) { alert(e); }
};

const fetchTasks = async () => {
  try {
    const res = await fetch('http://localhost:54321/task/list', { headers: { 'X-Ter-Token': agentToken.value } });
    if (res.ok) managedTasks.value = await res.json();
  } catch (e) { console.error(e); }
};

const startTask = async () => {
  if (!newTaskCmd.value) return;
  const parts = newTaskCmd.value.trim().split(/\s+/);
  try {
    const res = await fetch('http://localhost:54321/task/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Ter-Token': agentToken.value },
      body: JSON.stringify({ id: 'task-' + Date.now(), command: parts[0], args: parts.slice(1) })
    });
    if (res.ok) { newTaskCmd.value = ''; showAddTask.value = false; fetchTasks(); }
  } catch (e) { alert(e); }
};

const stopTask = async (id: string) => {
  try { await fetch(`http://localhost:54321/task/stop?id=${id}`, { headers: { 'X-Ter-Token': agentToken.value } }); fetchTasks(); } catch (e) { console.error(e); }
};

const viewLogs = async (id: string) => {
  try {
    const res = await fetch(`http://localhost:54321/task/logs?id=${id}`, { headers: { 'X-Ter-Token': agentToken.value } });
    if (res.ok) { selectedTaskLog.value = await res.text(); showLogModal.value = true; }
  } catch (e) { console.error(e); }
};

const fetchStats = async () => {
  try {
    const res = await fetch('http://localhost:54321/stats', { headers: { 'X-Ter-Token': agentToken.value } });
    if (res.ok) { stats.value = await res.json(); updateCharts(stats.value); }
  } catch (e) { console.error(e); }
};

const fetchFiles = async (path: string) => {
  try { 
    currentPath.value = path; 
    const files = await invoke('ls_remote', { path });
    // Sort: directories first, then files
    (files as any[]).sort((a, b) => {
      if (a.is_dir === b.is_dir) return a.name.localeCompare(b.name);
      return a.is_dir ? -1 : 1;
    });
    fileList.value = files as any[];
  } catch (e) { 
    console.error('ls_remote failed:', e); 
    errorMsg.value = 'Failed to list files: ' + e;
  }
};

const handleFileClick = async (f: any) => {
  try {
    const path = currentPath.value === '.' ? f.name : (currentPath.value.endsWith('/') ? currentPath.value + f.name : currentPath.value + '/' + f.name);
    if (f.is_dir) { 
      await fetchFiles(path); 
    } else {
      const localPath = await save({ defaultPath: f.name });
      if (localPath) await invoke('download_file', { remotePath: path, localPath });
    }
  } catch (e) {
    console.error('File click failed:', e);
    alert('Error: ' + e);
  }
};

const goBack = () => {
  if (currentPath.value === '.' || currentPath.value === '/') return;
  const parts = currentPath.value.split('/').filter(p => p);
  parts.pop();
  const newPath = parts.length === 0 ? '.' : '/' + parts.join('/');
  fetchFiles(newPath);
};

const uploadFile = async () => {
  try {
    const selected = await open({ multiple: false, directory: false });
    if (selected && typeof selected === 'string') {
      const filename = selected.includes('/') ? selected.split('/').pop() : selected.split('\\').pop();
      const remotePath = currentPath.value === '.' ? filename : (currentPath.value.endsWith('/') ? currentPath.value + filename : currentPath.value + '/' + filename);
      await invoke('upload_file', { localPath: selected, remotePath });
      await fetchFiles(currentPath.value);
    }
  } catch (e) {
    alert('Upload failed: ' + e);
  }
};

const killProcess = async (pid: number) => {
  if (!confirm(`Kill ${pid}?`)) return;
  try { await fetch(`http://localhost:54321/proc/kill?pid=${pid}`, { headers: { 'X-Ter-Token': agentToken.value } }); fetchStats(); } catch (e) { console.error(e); }
};

const formatBytes = (bytes: number) => {
  if (!bytes) return '0 B';
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(1) + ' ' + ['B', 'KB', 'MB', 'GB'][i];
};

const toggleDashboard = () => {
  showDashboard.value = !showDashboard.value;
  setTimeout(() => { cpuChart?.resize(); memChart?.resize(); fitAddon?.fit(); }, 200);
};
</script>

<template>
  <div class="app-container">
    <!-- Master Password Setup -->
    <div v-if="!isMasterPasswordSet" class="unlock-overlay">
      <div class="unlock-card">
        <div class="icon-header">🔒</div>
        <h2>Unlock Ter</h2>
        <p>Access your secure server vault</p>
        <div class="form-item">
          <label>Master Password</label>
          <input v-model="masterPassword" type="password" placeholder="••••••••" @keyup.enter="setMasterPass" :disabled="isProcessing" />
        </div>
        <button class="primary-btn" @click="setMasterPass" :disabled="isProcessing">
          <span v-if="!isProcessing">Unlock Vault</span>
          <span v-else class="loader"></span>
        </button>
        <p v-if="errorMsg" class="error-text">{{ errorMsg }}</p>
      </div>
    </div>

    <!-- Login/Server List Panel -->
    <div v-else-if="!isConnected" class="login-panel">
      <div class="server-mgmt" v-if="!showAddServer">
        <div class="mgmt-header">
          <h2>Server Vault</h2>
          <button class="add-btn" @click="showAddServer = true">+ Add</button>
        </div>
        <div class="server-grid">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)" :class="{ disabled: isProcessing }">
            <div class="s-info">
              <span class="s-label">{{ s.label }}</span>
              <span class="s-host">{{ s.user }}@{{ s.host }}</span>
            </div>
            <button class="del-btn" @click.stop="deleteServer(s.id)">✕</button>
          </div>
        </div>
        <div v-if="errorMsg" class="error-toast">{{ errorMsg }}</div>
      </div>

      <div v-else class="add-form">
        <h3>New Server</h3>
        <div class="form-grid">
          <div class="form-item">
            <label>Server Name</label>
            <input v-model="label" placeholder="e.g. Home Server" :disabled="isProcessing" />
          </div>
          <div class="form-item">
            <label>Host / IP Address</label>
            <input v-model="host" placeholder="192.168.1.x" :disabled="isProcessing" />
          </div>
          <div class="form-item">
            <label>Username</label>
            <input v-model="user" placeholder="root" :disabled="isProcessing" />
          </div>
          <div class="form-item">
            <label>Password</label>
            <input v-model="password" type="password" placeholder="••••••••" :disabled="isProcessing" />
          </div>
        </div>
        <div class="form-ops">
          <button @click="saveServer" class="primary-btn" :disabled="isProcessing">
            <span v-if="!isProcessing">Save & Exit</span>
            <span v-else class="loader"></span>
          </button>
          <button @click="connect" class="ghost-btn" :disabled="isProcessing">
            <span v-if="!isProcessing">Connect Now</span>
            <span v-else class="loader"></span>
          </button>
          <button @click="showAddServer = false" class="cancel-btn" :disabled="isProcessing">Cancel</button>
        </div>
        <div v-if="errorMsg" class="error-text">{{ errorMsg }}</div>
      </div>
    </div>

    <!-- Main Interface -->
    <div v-else class="main-layout">
      <aside :class="['sidebar', { collapsed: !showDashboard }]">
        <div class="sidebar-header">
          <span v-if="showDashboard" class="brand">⚡ System</span>
          <button class="toggle-btn" @click="toggleDashboard">{{ showDashboard ? '«' : '»' }}</button>
        </div>
        <div v-if="showDashboard" class="sidebar-scroll">
          <div v-if="stats" class="widget">
            <div class="widget-header">
              <label>Resource Usage</label>
              <small>{{ formatBytes(stats.mem_used) }}</small>
            </div>
            <div class="chart-box" ref="cpuChartRef"></div>
            <div class="chart-box" ref="memChartRef"></div>
          </div>
          <div class="widget">
            <div class="widget-header">
              <label>Files</label>
              <div class="file-ops">
                <button v-if="currentPath !== '.' && currentPath !== '/'" @click="goBack" class="mini-btn">⤴ Back</button>
                <button @click="uploadFile" class="mini-btn">Upload</button>
              </div>
            </div>
            <div class="current-path">{{ currentPath }}</div>
            <ul class="file-list">
              <li v-for="f in fileList" :key="f.name" @click="handleFileClick(f)">
                <span class="f-item">
                  <span class="f-icon">{{ f.is_dir ? '📁' : '📄' }}</span>
                  <span class="f-name">{{ f.name }}</span>
                </span>
                <span class="f-size">{{ f.is_dir ? '' : formatBytes(f.size) }}</span>
              </li>
            </ul>
          </div>
          <div class="widget">
            <div class="widget-header"><label>Tasks</label><button @click="showAddTask = !showAddTask">+</button></div>
            <input v-if="showAddTask" v-model="newTaskCmd" placeholder="Cmd..." @keyup.enter="startTask" class="mini-input" />
            <ul class="task-list">
              <li v-for="t in managedTasks" :key="t.id">
                <span class="t-name">{{ t.command }}</span>
                <div class="t-ops">
                  <button @click="viewLogs(t.id)">📜</button>
                  <button v-if="t.status === 'running'" @click="stopTask(t.id)">🛑</button>
                </div>
              </li>
            </ul>
          </div>
          <div class="widget procs">
            <div class="widget-header"><label>Processes</label></div>
            <ul class="proc-list">
              <li v-for="p in stats?.processes" :key="p.pid">
                <span>{{ p.name }}</span>
                <button @click="killProcess(p.pid)">Kill</button>
              </li>
            </ul>
          </div>
          <div class="ai-card" @click="showAiPanel = true">
            <span>✨ AI Sidekick</span>
            <small>{{ isAiInitialized ? 'Ready' : 'Initialize' }}</small>
          </div>
        </div>
      </aside>

      <main class="content">
        <header class="top-bar">
          <div class="status"><span class="led"></span> {{ user }}@{{ host }}</div>
          <div class="ops">
            <button @click="explainTerminalError" class="ai-btn">✨ Explain Error</button>
            <button @click="showGui = !showGui" class="gui-btn">🖥️ GUI</button>
          </div>
        </header>
        <div class="terminal-container-main">
          <div class="term-wrapper" ref="terminalRef"></div>
        </div>
      </main>

      <Transition name="slide">
        <div v-if="showAiPanel" class="ai-panel">
          <div class="panel-header"><h3>AI Sidekick</h3><button @click="showAiPanel = false">✕</button></div>
          <div v-if="!isAiInitialized" class="ai-init">
            <p>{{ aiProgress }}</p>
            <button v-if="!aiLoading" @click="initAi">Start Engine</button>
          </div>
          <div v-else class="chat">
            <div class="messages" ref="chatRef">
              <div v-for="(m, i) in aiChatHistory" :key="i" :class="['bubble', m.role]">{{ m.content }}</div>
            </div>
            <input v-model="userMessage" @keyup.enter="sendToAi()" placeholder="Ask anything..." />
          </div>
        </div>
      </Transition>

      <div v-if="showLogModal" class="modal">
        <div class="modal-content">
          <div class="modal-header">
            <h3>Logs</h3>
            <div class="modal-ops"><button @click="explainLogs">✨ Explain</button><button @click="showLogModal = false">✕</button></div>
          </div>
          <pre>{{ selectedTaskLog }}</pre>
        </div>
      </div>

      <div v-if="showGui" class="gui-overlay">
        <div class="gui-header"><h3>Remote Desktop</h3><button @click="showGui = false">✕</button></div>
        <div class="gui-body">
          <div v-if="!guiStatus.running" class="gui-off">
            <p>Environment not running.</p>
            <button @click="initGui">Launch Fluxbox</button>
          </div>
          <div v-else class="gui-on">
            <p>VNC Active on <code>localhost:55901</code></p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-container { height: 100vh; background: #09090b; color: #fafafa; font-family: 'Inter', system-ui, sans-serif; overflow: hidden; }

/* Shared UI Components */
label { display: block; font-size: 11px; font-weight: 600; color: #71717a; text-transform: uppercase; margin-bottom: 6px; letter-spacing: 0.02em; }
input { background: #09090b; border: 1px solid #27272a; padding: 12px; color: white; border-radius: 8px; font-size: 14px; transition: all 0.2s; width: 100%; box-sizing: border-box; }
input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.2); }
input:disabled { opacity: 0.5; cursor: not-allowed; }

button { cursor: pointer; transition: all 0.2s; border: none; font-weight: 600; display: flex; align-items: center; justify-content: center; }
button:active { transform: scale(0.96); }
button:disabled { opacity: 0.6; cursor: not-allowed; transform: none !important; }

.primary-btn { background: #6366f1; color: white; padding: 12px 20px; border-radius: 8px; font-size: 14px; }
.primary-btn:hover:not(:disabled) { background: #4f46e5; box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3); }

.ghost-btn { background: #1e1e2e; color: #a1a1aa; border: 1px solid #313244; padding: 12px 20px; border-radius: 8px; }
.ghost-btn:hover:not(:disabled) { border-color: #6366f1; color: white; }

.cancel-btn { background: transparent; color: #71717a; padding: 12px 20px; border-radius: 8px; }
.cancel-btn:hover:not(:disabled) { color: #fafafa; }

/* Unlock & Login Panels */
.unlock-overlay, .login-panel { display: flex; align-items: center; justify-content: center; height: 100%; flex-direction: column; background: radial-gradient(circle at center, #121215 0%, #09090b 100%); }

.unlock-card { background: #18181b; padding: 40px; border-radius: 20px; border: 1px solid #27272a; width: 380px; text-align: center; box-shadow: 0 20px 40px rgba(0,0,0,0.4); }
.icon-header { font-size: 40px; margin-bottom: 15px; }
.unlock-card h2 { margin: 0 0 8px 0; font-size: 24px; }
.unlock-card p { color: #71717a; font-size: 14px; margin-bottom: 30px; }

.server-mgmt { width: 500px; display: flex; flex-direction: column; gap: 24px; }
.mgmt-header { display: flex; justify-content: space-between; align-items: center; }
.add-btn { background: #27272a; padding: 6px 14px; border-radius: 6px; font-size: 13px; color: #a1a1aa; }
.add-btn:hover { background: #3f3f46; color: white; }

.server-grid { display: flex; flex-direction: column; gap: 12px; max-height: 400px; overflow-y: auto; padding-right: 5px; }
.server-card { background: #18181b; border: 1px solid #27272a; padding: 16px; border-radius: 12px; display: flex; justify-content: space-between; align-items: center; cursor: pointer; }
.server-card:hover:not(.disabled) { border-color: #6366f1; background: #1e1e2e; }
.s-info { display: flex; flex-direction: column; gap: 4px; }
.s-label { font-weight: 700; font-size: 15px; }
.s-host { font-size: 12px; color: #71717a; font-family: monospace; }
.del-btn { background: transparent; color: #52525b; font-size: 16px; width: 32px; height: 32px; border-radius: 50%; }
.del-btn:hover { background: #ef4444; color: white; }

.add-form { background: #18181b; padding: 32px; border-radius: 16px; border: 1px solid #27272a; width: 420px; box-shadow: 0 20px 50px rgba(0,0,0,0.5); }
.add-form h3 { margin: 0 0 24px 0; font-size: 20px; }
.form-grid { display: grid; grid-template-columns: 1fr; gap: 16px; margin-bottom: 32px; }
.form-ops { display: flex; flex-direction: column; gap: 10px; }

/* Feedback */
.error-text { color: #ef4444; font-size: 13px; margin-top: 15px; text-align: center; }
.error-toast { background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); color: #ef4444; padding: 12px; border-radius: 8px; font-size: 13px; text-align: center; }

/* Loader Animation */
.loader { width: 18px; height: 18px; border: 2px solid rgba(255,255,255,0.3); border-radius: 50%; border-top-color: #fff; animation: spin 0.8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

/* Main UI (Retained & Integrated) */
.main-layout { display: flex; height: 100%; }
.sidebar { width: 260px; background: #121215; border-right: 1px solid #27272a; display: flex; flex-direction: column; transition: 0.2s; }
.sidebar.collapsed { width: 50px; }
.sidebar-header { padding: 15px; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #27272a; }
.sidebar-scroll { flex: 1; overflow-y: auto; padding: 15px; display: flex; flex-direction: column; gap: 15px; }
.widget { background: #18181b; border: 1px solid #27272a; padding: 10px; border-radius: 8px; }
.widget-header { display: flex; justify-content: space-between; align-items: center; font-size: 11px; color: #71717a; text-transform: uppercase; margin-bottom: 8px; }
.file-ops { display: flex; gap: 5px; }
.mini-btn { font-size: 10px; padding: 2px 8px; background: #27272a; border-radius: 4px; color: #a1a1aa; }
.mini-btn:hover { background: #3f3f46; color: white; }
.current-path { font-size: 10px; color: #71717a; margin-bottom: 5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: monospace; border-bottom: 1px solid #27272a; padding-bottom: 2px; }

.chart-box { height: 60px; }
.file-list, .task-list, .proc-list { list-style: none; padding: 0; font-size: 12px; max-height: 180px; overflow-y: auto; }
.file-list li, .task-list li, .proc-list li { padding: 6px 8px; cursor: pointer; display: flex; justify-content: space-between; align-items: center; border-radius: 6px; transition: 0.2s; }
.file-list li:hover, .task-list li:hover, .proc-list li:hover { background: #1e1e2e; }

.f-item { display: flex; align-items: center; gap: 8px; overflow: hidden; }
.f-icon { font-size: 14px; flex-shrink: 0; }
.f-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.f-size { font-size: 10px; color: #52525b; flex-shrink: 0; }
.ai-card { background: #1e1e2e; padding: 12px; border-radius: 8px; cursor: pointer; display: flex; flex-direction: column; }
.content { flex: 1; display: flex; flex-direction: column; }
.top-bar { height: 50px; background: #121215; border-bottom: 1px solid #27272a; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; }
.led { width: 8px; height: 8px; background: #22c55e; border-radius: 50%; display: inline-block; margin-right: 10px; }
.terminal-container-main { flex: 1; padding: 20px; background: #09090b; }
.term-wrapper { height: 100%; background: black; border-radius: 8px; border: 1px solid #27272a; padding: 10px; }
.ai-panel { position: absolute; right: 0; top: 0; bottom: 0; width: 350px; background: #121215; border-left: 1px solid #27272a; z-index: 100; display: flex; flex-direction: column; }
.panel-header { padding: 15px; border-bottom: 1px solid #27272a; display: flex; justify-content: space-between; }
.chat { flex: 1; display: flex; flex-direction: column; padding: 15px; gap: 10px; }
.messages { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 8px; }
.bubble { padding: 8px; border-radius: 6px; font-size: 13px; max-width: 90%; }
.bubble.user { background: #6366f1; align-self: flex-end; }
.bubble.assistant { background: #27272a; align-self: flex-start; }
.modal { position: fixed; inset: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 200; }
.modal-content { background: #18181b; width: 80%; height: 80%; border-radius: 12px; display: flex; flex-direction: column; }
pre { flex: 1; padding: 20px; overflow: auto; background: #09090b; margin: 0; font-family: monospace; font-size: 12px; }
.gui-overlay { position: fixed; inset: 50px 0 0 260px; background: #000; z-index: 50; }
.gui-header { padding: 10px; background: #121215; display: flex; justify-content: space-between; }
.gui-body { display: flex; align-items: center; justify-content: center; height: calc(100% - 40px); }
</style>