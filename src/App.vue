<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
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
    cpuChart = echarts.init(cpuChartRef.value);
    cpuChart.setOption(getChartOption('CPU Usage (%)', '#6366f1'));
  }
  if (memChartRef.value) {
    memChart = echarts.init(memChartRef.value);
    memChart.setOption(getChartOption('Memory Usage (%)', '#a855f7'));
  }
};

const getChartOption = (title: string, color: string) => ({
  title: { text: title, textStyle: { color: '#71717a', fontSize: 10, fontWeight: 'normal' }, left: 'center' },
  grid: { top: 25, bottom: 5, left: 30, right: 5 },
  xAxis: { type: 'category', show: false },
  yAxis: { type: 'value', min: 0, max: 100, splitLine: { lineStyle: { color: '#27272a' } } },
  series: [{ data: [], type: 'line', smooth: true, showSymbol: false, areaStyle: { color }, itemStyle: { color } }],
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
  if (terminalRef.value) {
    term.open(terminalRef.value);
    try { term.loadAddon(new WebglAddon()); } catch (e) {}
    fitAddon.fit();
  }
  term.onData(async (data) => { if (isConnected.value) await invoke('write_pty', { data }); });
  term.onResize(async (size) => { if (isConnected.value) await invoke('resize_pty', { cols: size.cols, rows: size.rows }); });
  window.addEventListener('resize', () => { fitAddon?.fit(); cpuChart?.resize(); memChart?.resize(); });
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
  try {
    await invoke('set_master_password', { password: masterPassword.value });
    isMasterPasswordSet.value = true;
    loadServers();
  } catch (e) { errorMsg.value = 'Security Error: ' + e; }
};

const loadServers = async () => {
  try { savedServers.value = await invoke('list_server_configs'); } catch (e) { console.error(e); }
};

const saveServer = async () => {
  try {
    const config = { id: window.crypto.randomUUID(), label: label.value, host: host.value, user: user.value, port: 22, password_enc: password.value, key_path: null };
    await invoke('save_server_config', { config });
    showAddServer.value = false;
    loadServers();
  } catch (e) { errorMsg.value = String(e); }
};

const deleteServer = async (id: string) => {
  if (!confirm('Are you sure?')) return;
  try { await invoke('delete_server_config', { id }); loadServers(); } catch (e) { errorMsg.value = String(e); }
};

const connectWithId = async (id: string) => {
  try { await invoke('connect_with_id', { id }); onConnected(); } catch (e) { errorMsg.value = String(e); }
};

const connect = async () => {
  try { await invoke('connect_to_ssh', { host: host.value, user: user.value, pass: password.value }); onConnected(); } catch (e) { errorMsg.value = String(e); }
};

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  setTimeout(() => { fitAddon.fit(); invoke('resize_pty', { cols: term.cols, rows: term.rows }); }, 100);
  fetchFiles('.');
  term.focus();
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
  try { currentPath.value = path; fileList.value = await invoke('ls_remote', { path }); } catch (e) { console.error(e); }
};

const handleFileClick = async (f: any) => {
  if (f.is_dir) { fetchFiles(currentPath.value + '/' + f.name); }
  else {
    const localPath = await save({ defaultPath: f.name });
    if (localPath) await invoke('download_file', { remotePath: currentPath.value + '/' + f.name, localPath });
  }
};

const uploadFile = async () => {
  const selected = await open({ multiple: false, directory: false });
  if (selected && typeof selected === 'string') {
    await invoke('upload_file', { localPath: selected, remotePath: currentPath.value + '/' + selected.split('/').pop() });
    fetchFiles(currentPath.value);
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
    <div v-if="!isMasterPasswordSet" class="unlock-overlay">
      <div class="unlock-card">
        <h2>Unlock Ter</h2>
        <input v-model="masterPassword" type="password" placeholder="Master Password" @keyup.enter="setMasterPass" />
        <button class="primary-btn" @click="setMasterPass">Unlock Vault</button>
      </div>
    </div>

    <div v-else-if="!isConnected" class="login-panel">
      <div class="server-mgmt" v-if="!showAddServer">
        <h2>Servers</h2>
        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <span>{{ s.label }}</span>
            <small>{{ s.user }}@{{ s.host }}</small>
            <button @click.stop="deleteServer(s.id)">✕</button>
          </div>
        </div>
        <button class="add-btn" @click="showAddServer = true">+ Add New</button>
      </div>
      <div v-else class="add-form">
        <input v-model="label" placeholder="Label" />
        <input v-model="host" placeholder="Host" />
        <input v-model="user" placeholder="User" />
        <input v-model="password" type="password" placeholder="Password" />
        <div class="form-ops">
          <button @click="saveServer">Save</button>
          <button @click="connect" class="ghost">Connect Once</button>
          <button @click="showAddServer = false" class="ghost">Cancel</button>
        </div>
      </div>
    </div>

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
            <div class="widget-header"><label>Files</label><button @click="uploadFile">Upload</button></div>
            <ul class="file-list">
              <li v-for="f in fileList" :key="f.name" @click="handleFileClick(f)">
                <span>{{ f.is_dir ? '📁' : '📄' }}</span> {{ f.name }}
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
.app-container { height: 100vh; background: #09090b; color: #fafafa; font-family: Inter, sans-serif; overflow: hidden; }
.unlock-overlay, .login-panel { display: flex; align-items: center; justify-content: center; height: 100%; flex-direction: column; gap: 20px; }
.unlock-card, .add-form { background: #18181b; padding: 40px; border-radius: 12px; border: 1px solid #27272a; width: 350px; display: flex; flex-direction: column; gap: 15px; }
input { background: #09090b; border: 1px solid #27272a; padding: 10px; color: white; border-radius: 6px; }
button { background: #6366f1; color: white; border: none; padding: 10px; border-radius: 6px; cursor: pointer; }
button.ghost { background: transparent; border: 1px solid #27272a; }
.main-layout { display: flex; height: 100%; }
.sidebar { width: 260px; background: #121215; border-right: 1px solid #27272a; display: flex; flex-direction: column; transition: 0.2s; }
.sidebar.collapsed { width: 50px; }
.sidebar-header { padding: 15px; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #27272a; }
.sidebar-scroll { flex: 1; overflow-y: auto; padding: 15px; display: flex; flex-direction: column; gap: 15px; }
.widget { background: #18181b; border: 1px solid #27272a; padding: 10px; border-radius: 8px; }
.widget-header { display: flex; justify-content: space-between; font-size: 11px; color: #71717a; text-transform: uppercase; margin-bottom: 8px; }
.chart-box { height: 60px; }
.file-list, .task-list, .proc-list { list-style: none; padding: 0; font-size: 12px; }
.file-list li, .task-list li, .proc-list li { padding: 5px; cursor: pointer; display: flex; justify-content: space-between; }
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