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
const isProcessing = ref(false);
const showDashboard = ref(true);
const cyberMode = ref(false);
const backendLogs = ref<string[]>([]);
const errorMsg = ref('');

const MAX_LOGS = 100;

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

const aiEngine = ref<any>(null);
const aiLoading = ref(false);
const aiProgress = ref('');
const aiChatHistory = ref<{ role: string, content: string }[]>([]);
const userMessage = ref('');
const isAiInitialized = ref(false);
const showAiPanel = ref(false);
const localModelPath = ref('');
const aiStats = ref<any>(null);
const chatRef = ref<HTMLElement | null>(null);

const sidebarWidth = ref(260);
const isResizing = ref(false);
const terminalInputBuffer = ref('');
const fontSize = ref(14);

const showContextMenu = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const selectedText = ref('');

const MODEL_ID = "SmolLM2-135M-Instruct-v0.1-q4f16_1-MLC";

const addLog = (msg: string) => {
  backendLogs.value.push(msg);
  if (backendLogs.value.length > MAX_LOGS) backendLogs.value.shift();
};

const startResizing = (_e: MouseEvent) => {
  isResizing.value = true;
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResizing);
  document.body.style.cursor = 'col-resize';
};

const handleMouseMove = (e: MouseEvent) => {
  if (!isResizing.value) return;
  const newWidth = e.clientX;
  if (newWidth > 150 && newWidth < 600) {
    sidebarWidth.value = newWidth;
    nextTick(() => { onResize(); });
  }
};

const stopResizing = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResizing);
  document.body.style.cursor = 'default';
};

const handleWheel = (e: WheelEvent) => {
  if (e.ctrlKey) {
    e.preventDefault();
    const delta = e.deltaY > 0 ? -1 : 1;
    const newSize = Math.min(Math.max(fontSize.value + delta, 8), 40);
    if (newSize !== fontSize.value) {
      fontSize.value = newSize;
      term.options.fontSize = fontSize.value;
      nextTick(() => { fitAddon.fit(); });
    }
  }
};

const onTerminalContextMenu = (e: MouseEvent) => {
  e.preventDefault();
  const selection = term.getSelection();
  if (selection) {
    selectedText.value = selection;
    menuPos.value = { x: e.clientX, y: e.clientY };
    showContextMenu.value = true;
  }
};

const runAsTask = async () => {
  if (!selectedText.value) return;
  const parts = selectedText.value.trim().split(/\s+/);
  try {
    const res = await fetch('http://localhost:54321/task/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Ter-Token': agentToken.value },
      body: JSON.stringify({ id: 'task-' + Date.now(), command: parts[0], args: parts.slice(1) })
    });
    if (res.ok) {
      addLog(`[SYSTEM] Background task started: ${selectedText.value}`);
      showContextMenu.value = false;
      fetchTasks();
    }
  } catch (e) { alert(e); }
};

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

const updateCharts = (statsVal: any) => {
  const now = new Date().toLocaleTimeString();
  const memPerc = (statsVal.mem_used / statsVal.mem_total) * 100;
  cpuHistory.value.push(statsVal.cpu_usage);
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

const onResize = () => { 
  if (isConnected.value) {
    fitAddon?.fit(); 
    cpuChart?.resize(); 
    memChart?.resize(); 
  }
};

const scrollToBottom = () => {
  setTimeout(() => {
    if (chatRef.value) {
      chatRef.value.scrollTop = chatRef.value.scrollHeight;
    }
  }, 100);
};

let term: Terminal;
let fitAddon: FitAddon;
let unlistenPty: (() => void) | null = null;
let statsInterval: number | null = null;
let onDataListener: { dispose: () => void } | null = null;
let onResizeListener: { dispose: () => void } | null = null;
let unlistenLog: (() => void) | null = null;

onMounted(async () => {
  term = new Terminal({
    cursorBlink: true,
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Roboto Mono', monospace",
    fontSize: fontSize.value,
    letterSpacing: 0.5,
    lineHeight: 1.2,
    theme: { background: '#000', foreground: '#fafafa' },
    allowTransparency: true
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  
  onDataListener = term.onData(async (data) => { 
    if (isConnected.value) {
      if (data === '\r') {
        if (terminalInputBuffer.value.trim() === 'cyberon') cyberMode.value = true;
        else if (terminalInputBuffer.value.trim() === 'cyberoff') cyberMode.value = false;
        terminalInputBuffer.value = '';
      } else if (data === '\u007f') terminalInputBuffer.value = terminalInputBuffer.value.slice(0, -1);
      else terminalInputBuffer.value += data;
      await invoke('write_pty', { data }); 
    }
  });
  onResizeListener = term.onResize(async (size) => { if (isConnected.value) await invoke('resize_pty', { cols: size.cols, rows: size.rows }); });
  window.addEventListener('resize', onResize);

  unlistenLog = await listen<string>('backend-log', (event) => {
    backendLogs.value.push(event.payload);
    if (backendLogs.value.length > MAX_LOGS) backendLogs.value.shift();
  });

  const savedPath = await invoke('get_model_path');
  if (savedPath) localModelPath.value = savedPath as string;
});

onUnmounted(() => {
  if (unlistenPty) unlistenPty();
  if (unlistenLog) unlistenLog();
  if (statsInterval) clearInterval(statsInterval);
  if (onDataListener) onDataListener.dispose();
  if (onResizeListener) onResizeListener.dispose();
  window.removeEventListener('resize', onResize);
  term?.dispose();
  cpuChart?.dispose();
  memChart?.dispose();
});

const selectModelFolder = async () => {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (selected && typeof selected === 'string') {
      await invoke('set_model_path', { path: selected });
      localModelPath.value = selected;
      addLog(`[SYSTEM] Model path updated: ${selected}`);
    }
  } catch (e) { alert(String(e)); }
};

const initAi = async () => {
  if (isAiInitialized.value) return;
  aiLoading.value = true;
  try {
    const appConfig: any = {
      model_list: [{
        model_id: MODEL_ID,
        model: MODEL_ID,
        model_lib: `${webllm.modelLibURLPrefix}SmolLM2-135M-Instruct-v0.1-q4f16_1-MLC-webgpu.wasm`,
        model_url: `ter-model://localhost/`,
        low_resource_required: true,
      }],
    };
    
    const engine = await webllm.CreateMLCEngine(MODEL_ID, { 
      appConfig,
      initProgressCallback: (p) => { 
        const perc = Math.round(p.progress * 100);
        aiProgress.value = `Loading: ${perc}%`;
        addLog(`[NEURAL] Weight sequence: ${perc}% | ${p.text}`);
      } 
    });
    aiEngine.value = engine;
    isAiInitialized.value = true;
    aiProgress.value = 'AI Online';
    addLog(`[NEURAL] Core engaged: ${MODEL_ID}`);
  } catch (e) { addLog(`[NEURAL] Core failure: ${e}`); aiProgress.value = 'Error: ' + e; } finally { aiLoading.value = false; }
};

const sendToAi = async (msg: string = '') => {
  const content = msg || userMessage.value;
  if (!content || !aiEngine.value) return;
  aiChatHistory.value.push({ role: 'user', content });
  userMessage.value = ''; aiLoading.value = true; scrollToBottom();

  const assistantIdx = aiChatHistory.value.push({ role: 'assistant', content: '' }) - 1;

  try {
    const chunks = await aiEngine.value.chat.completions.create({ 
      messages: aiChatHistory.value.slice(0, -1) as any, 
      stream: true 
    });
    
    let fullReply = "";
    for await (const chunk of chunks) {
      const delta = chunk.choices[0]?.delta?.content || "";
      fullReply += delta;
      if (aiChatHistory.value[assistantIdx]) {
        aiChatHistory.value[assistantIdx].content = fullReply;
      }
      scrollToBottom();
      
      const rtStats = await aiEngine.value.runtimeStats();
      if (rtStats) {
        aiStats.value = rtStats;
        addLog(`[NEURAL] Synapse firing: ${rtStats.decodeTokensPerSec.toFixed(1)} t/s`);
      }
    }
  } catch (e) { 
    if (aiChatHistory.value[assistantIdx]) {
      aiChatHistory.value[assistantIdx].content = 'Error: ' + e; 
    }
  } finally { aiLoading.value = false; }
};

const fetchTasks = async () => {
  if (!agentToken.value || !isConnected.value) return;
  try {
    const res = await fetch('http://localhost:54321/task/list', { 
      headers: { 'X-Ter-Token': agentToken.value },
      signal: AbortSignal.timeout(1500) 
    });
    if (res.ok) managedTasks.value = await res.json();
  } catch (e) { console.debug(e); }
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
  } catch (e) { alert(String(e)); }
};

const stopTask = async (id: string) => {
  try { await fetch(`http://localhost:54321/task/stop?id=${id}`, { headers: { 'X-Ter-Token': agentToken.value } }); fetchTasks(); } catch (e) { console.debug(e); }
};

const viewLogs = async (id: string) => {
  try {
    const res = await fetch(`http://localhost:54321/task/logs?id=${id}`, { headers: { 'X-Ter-Token': agentToken.value } });
    if (res.ok) { selectedTaskLog.value = await res.text(); showLogModal.value = true; }
  } catch (e) { console.debug(e); }
};

const fetchStats = async () => {
  if (!agentToken.value || !isConnected.value) return;
  try {
    const res = await fetch('http://localhost:54321/stats', { 
      headers: { 'X-Ter-Token': agentToken.value },
      signal: AbortSignal.timeout(1500)
    });
    if (res.ok) { stats.value = await res.json(); updateCharts(stats.value); }
  } catch (e) { console.debug(e); }
};

const onConnected = async () => {
  if (unlistenPty) { unlistenPty(); unlistenPty = null; }
  if (statsInterval) { clearInterval(statsInterval); statsInterval = null; }
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  await nextTick();
  if (terminalRef.value) {
    term.open(terminalRef.value);
    try { term.loadAddon(new WebglAddon()); } catch (e) { console.debug(e); }
    setTimeout(() => {
      fitAddon.fit();
      invoke('resize_pty', { cols: term.cols, rows: term.rows });
      term.focus();
    }, 150);
  }
  fetchFiles('.');
  unlistenPty = await listen<number[]>('pty-data', (event) => { term.write(new Uint8Array(event.payload)); });
  statsInterval = window.setInterval(() => { fetchStats(); fetchTasks(); }, 2000);
  initCharts();
};

const fetchFiles = async (path: string) => {
  try { 
    currentPath.value = path; 
    const files = await invoke('ls_remote', { path });
    (files as any[]).sort((a, b) => {
      if (a.is_dir === b.is_dir) return a.name.localeCompare(b.name);
      return a.is_dir ? -1 : 1;
    });
    fileList.value = files as any[];
  } catch (e) { errorMsg.value = 'Failed to list files: ' + e; }
};

const handleFileClick = async (f: any) => {
  const path = currentPath.value === '.' ? f.name : (currentPath.value.endsWith('/') ? currentPath.value + f.name : currentPath.value + '/' + f.name);
  if (f.is_dir) await fetchFiles(path); 
  else {
    const localPath = await save({ defaultPath: f.name });
    if (localPath) await invoke('download_file', { remotePath: path, localPath });
  }
};

const goBack = () => {
  if (currentPath.value === '.' || currentPath.value === '/') return;
  const parts = currentPath.value.split('/').filter(p => p);
  parts.pop();
  const newPath = parts.length === 0 ? '.' : '/' + parts.join('/');
  fetchFiles(newPath);
};

const killProcess = async (pid: number) => {
  if (!confirm(`Kill ${pid}?`)) return;
  try { await fetch(`http://localhost:54321/proc/kill?pid=${pid}`, { headers: { 'X-Ter-Token': agentToken.value } }); fetchStats(); } catch (e) { console.debug(e); }
};

const toggleDashboard = () => {
  showDashboard.value = !showDashboard.value;
  setTimeout(() => { cpuChart?.resize(); memChart?.resize(); fitAddon?.fit(); }, 200);
};

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
  try { savedServers.value = await invoke('list_server_configs'); } catch (e) { console.debug(e); }
};

const saveServer = async () => {
  isProcessing.value = true;
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
  try { await invoke('connect_with_id', { id }); await onConnected(); } catch (e) { errorMsg.value = String(e); }
  finally { isProcessing.value = false; }
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
</script>

<template>
  <div :class="['app-container', { 'cyber-mode': cyberMode }]" @contextmenu.prevent @click="showContextMenu = false">
    <div v-if="cyberMode" class="cyber-logs-layer">
      <div v-for="(log, i) in backendLogs" :key="i" class="cyber-log-line">{{ log }}</div>
    </div>

    <div v-if="!isMasterPasswordSet" class="unlock-overlay">
      <div class="unlock-card">
        <h2>Unlock Ter</h2>
        <input v-model="masterPassword" type="password" placeholder="Master Password" @keyup.enter="setMasterPass" />
        <button class="primary-btn" @click="setMasterPass" :disabled="isProcessing">Unlock</button>
      </div>
    </div>

    <div v-else-if="!isConnected" class="login-panel">
      <div class="server-mgmt" v-if="!showAddServer">
        <div class="mgmt-header"><h2>Server Vault</h2><button @click="showAddServer = true">+</button></div>
        <div class="server-grid">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <span>{{ s.label }} ({{ s.user }}@{{ s.host }})</span>
            <button @click.stop="deleteServer(s.id)">✕</button>
          </div>
        </div>
      </div>
      <div v-else class="add-form">
        <input v-model="label" placeholder="Label" /><input v-model="host" placeholder="Host" />
        <input v-model="user" placeholder="User" /><input v-model="password" type="password" placeholder="Pass" />
        <button @click="saveServer" class="primary-btn">Save</button>
        <button @click="showAddServer = false">Cancel</button>
      </div>
    </div>

    <div v-else class="main-layout" :style="{ '--sidebar-width': sidebarWidth + 'px' }">
      <aside :class="['sidebar', { collapsed: !showDashboard }]">
        <div class="sidebar-header">
          <span v-if="showDashboard">⚡ System</span>
          <button @click="toggleDashboard">{{ showDashboard ? '«' : '»' }}</button>
        </div>
        <div v-if="showDashboard" class="sidebar-scroll">
          <div class="widget">
            <div class="chart-box" ref="cpuChartRef"></div>
            <div class="chart-box" ref="memChartRef"></div>
          </div>
          <div class="widget">
            <div class="widget-header"><label>Files</label><button @click="goBack">⤴</button></div>
            <ul class="file-list">
              <li v-for="f in fileList" :key="f.name" @click="handleFileClick(f)">
                {{ f.is_dir ? '📁' : '📄' }} {{ f.name }}
              </li>
            </ul>
          </div>
          <div class="widget">
            <div class="widget-header"><label>Tasks</label><button @click="showAddTask = !showAddTask">+</button></div>
            <input v-if="showAddTask" v-model="newTaskCmd" placeholder="Cmd..." @keyup.enter="startTask" />
            <ul class="task-list">
              <li v-for="t in managedTasks" :key="t.id">
                <span>{{ t.command }}</span>
                <button @click="viewLogs(t.id)">📜</button>
                <button v-if="t.status === 'running'" @click="stopTask(t.id)">🛑</button>
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
        </div>
      </aside>

      <div v-if="showDashboard" class="resizer" @mousedown="startResizing"></div>

      <main class="content">
        <header class="top-bar">
          <div class="status">{{ user }}@{{ host }}</div>
          <div class="ops">
            <button @click="cyberMode = !cyberMode" :class="{ active: cyberMode }">Cyber</button>
            <button @click="explainTerminalError" class="ai-btn">✨ AI Sidekick</button>
          </div>
        </header>
        <div class="terminal-container-main">
          <div class="term-wrapper" ref="terminalRef" @wheel="handleWheel" @contextmenu="onTerminalContextMenu"></div>
          <div v-if="showContextMenu" class="context-menu" :style="{ left: menuPos.x + 'px', top: menuPos.y + 'px' }">
            <button @click="runAsTask">🚀 Run as Background Task</button>
            <button @click="showContextMenu = false">Cancel</button>
          </div>
        </div>
      </main>

      <Transition name="slide">
        <div v-if="showAiPanel" class="ai-panel">
          <div class="panel-header">
            <h3>AI Sidekick</h3>
            <div class="header-ops">
              <button @click="selectModelFolder" class="mini-btn">📂</button>
              <button @click="showAiPanel = false">✕</button>
            </div>
          </div>
          <div v-if="!isAiInitialized" class="ai-init">
            <div class="guide-card">
              <h4>🤖 Neural Engine</h4>
              <p>On-device AI. Private & Secure.</p>
              <ul class="guide-list">
                <li>Click 📂 to select model folder</li>
                <li>Ensure <b>SmolLM2</b> weights are inside</li>
                <li>Click <b>Start Core</b></li>
              </ul>
            </div>
            <p v-if="localModelPath" class="path-info">📁 {{ localModelPath }}</p>
            <p v-else class="path-warn">⚠️ No model folder selected</p>
            <button v-if="!aiLoading" @click="initAi" :disabled="!localModelPath" class="primary-btn">Start Core</button>
            <div v-else class="loader"></div>
            <p class="progress-text">{{ aiProgress }}</p>
          </div>
          <div v-else class="chat">
            <div class="messages" ref="chatRef">
              <div v-for="(m, i) in aiChatHistory" :key="i" :class="['bubble', m.role]">{{ m.content }}</div>
            </div>
            <input v-model="userMessage" @keyup.enter="sendToAi()" placeholder="Ask anything..." />
          </div>
        </div>
      </Transition>
    </div>

    <div v-if="showLogModal" class="modal" @click.self="showLogModal = false">
      <div class="modal-content">
        <div class="modal-header"><h3>Logs</h3><button @click="showLogModal = false">✕</button></div>
        <pre>{{ selectedTaskLog }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-container { height: 100vh; background: #09090b; color: #fafafa; overflow: hidden; position: relative; }
.main-layout { display: flex; height: 100%; }
.sidebar { width: var(--sidebar-width, 260px); background: #121215; border-right: 1px solid #27272a; display: flex; flex-direction: column; flex-shrink: 0; }
.sidebar.collapsed { width: 50px; }
.resizer { width: 4px; cursor: col-resize; transition: background 0.2s; z-index: 10; flex-shrink: 0; }
.resizer:hover { background: #6366f1; }
.content { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.top-bar { height: 50px; background: #121215; border-bottom: 1px solid #27272a; display: flex; align-items: center; justify-content: space-between; padding: 0 20px; }
.terminal-container-main { flex: 1; padding: 24px; background: #09090b; position: relative; overflow: hidden; }
.term-wrapper { height: 100%; background: black; border-radius: 12px; border: 1px solid #27272a; padding: 16px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); }
.ai-panel { position: absolute; right: 0; top: 0; bottom: 0; width: 350px; background: #121215; border-left: 1px solid #27272a; z-index: 100; display: flex; flex-direction: column; }
.panel-header { padding: 15px; border-bottom: 1px solid #27272a; display: flex; justify-content: space-between; align-items: center; }
.ai-init { flex: 1; padding: 20px; display: flex; flex-direction: column; gap: 20px; align-items: center; justify-content: center; }
.guide-card { background: #18181b; border: 1px solid #27272a; padding: 16px; border-radius: 12px; font-size: 13px; width: 100%; }
.guide-list { margin: 10px 0 0 18px; padding: 0; color: #a1a1aa; }
.path-info { font-size: 11px; color: #6366f1; background: rgba(99, 102, 241, 0.1); padding: 8px; border-radius: 6px; width: 100%; word-break: break-all; }
.chat { flex: 1; display: flex; flex-direction: column; padding: 15px; overflow: hidden; }
.messages { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; margin-bottom: 10px; }
.bubble { padding: 10px; border-radius: 8px; font-size: 13px; max-width: 85%; }
.bubble.user { align-self: flex-end; background: #6366f1; }
.bubble.assistant { align-self: flex-start; background: #27272a; }
.progress-text { font-size: 11px; color: #71717a; }
.context-menu { position: fixed; background: #18181b; border: 1px solid #3f3f46; border-radius: 8px; padding: 4px; z-index: 1000; box-shadow: 0 10px 20px rgba(0,0,0,0.4); }
.context-menu button { width: 100%; padding: 8px 12px; text-align: left; background: transparent; color: #fafafa; font-size: 13px; border-radius: 4px; border: none; cursor: pointer; }
.context-menu button:hover { background: #6366f1; }
.widget { background: #18181b; border: 1px solid #27272a; padding: 10px; border-radius: 8px; margin-bottom: 10px; }
.chart-box { height: 60px; }
.file-list, .task-list, .proc-list { list-style: none; padding: 0; font-size: 12px; max-height: 200px; overflow-y: auto; }
.file-list li, .task-list li, .proc-list li { padding: 6px; cursor: pointer; border-radius: 4px; display: flex; justify-content: space-between; align-items: center; }
.file-list li:hover, .task-list li:hover, .proc-list li:hover { background: #27272a; }
.modal { position: fixed; inset: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 2000; }
.modal-content { background: #18181b; width: 80%; height: 80%; border-radius: 12px; display: flex; flex-direction: column; padding: 20px; }
pre { flex: 1; overflow: auto; background: #000; padding: 10px; font-family: monospace; font-size: 12px; }
.cyber-logs-layer { position: absolute; inset: 0; z-index: -1; padding: 20px; display: flex; flex-direction: column; justify-content: flex-end; background: #050505; }
.cyber-log-line { font-family: monospace; font-size: 10px; color: rgba(34, 197, 94, 0.4); }
input { background: #09090b; border: 1px solid #27272a; color: white; padding: 8px; border-radius: 4px; width: 100%; margin-bottom: 10px; }
.primary-btn { background: #6366f1; color: white; padding: 8px 16px; border-radius: 4px; width: 100%; cursor: pointer; border: none; }
.loader { width: 18px; height: 18px; border: 2px solid rgba(255,255,255,0.3); border-radius: 50%; border-top-color: #fff; animation: spin 0.8s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
