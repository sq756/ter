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
const showDashboard = ref(false);
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
    cpuChart.setOption(getChartOption('CPU Usage (%)', '#4caf50'));
  }
  if (memChartRef.value) {
    memChart = echarts.init(memChartRef.value);
    memChart.setOption(getChartOption('Memory Usage (%)', '#007acc'));
  }
};

const getChartOption = (title: string, color: string) => ({
  title: {
    text: title,
    textStyle: { color: '#aaa', fontSize: 11, fontWeight: 'normal' },
    left: 'center',
    top: 0
  },
  grid: { top: 30, bottom: 20, left: 35, right: 10 },
  xAxis: { type: 'category', data: [], show: false },
  yAxis: { type: 'value', min: 0, max: 100, splitLine: { lineStyle: { color: '#333' } } },
  series: [{
    data: [],
    type: 'line',
    smooth: true,
    showSymbol: false,
    areaStyle: { color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [{ offset: 0, color }, { offset: 1, color: 'transparent' }]) },
    itemStyle: { color }
  }],
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
  // Check if we need master password (simplified: always ask for now)
  
  term = new Terminal({
    cursorBlink: true,
    fontFamily: 'Menlo, Monaco, "Courier New", monospace',
    theme: { background: '#1e1e1e' }
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);

  if (terminalRef.value) {
    term.open(terminalRef.value);
    try {
      const webgl = new WebglAddon();
      term.loadAddon(webgl);
    } catch (e) {}
    fitAddon.fit();
  }

  term.onData(async (data) => {
    if (isConnected.value) {
      await invoke('write_pty', { data });
    }
  });

  term.onResize(async (size) => {
    if (isConnected.value) {
      await invoke('resize_pty', { cols: size.cols, rows: size.rows });
    }
  });

  window.addEventListener('resize', () => {
    fitAddon?.fit();
    cpuChart?.resize();
    memChart?.resize();
  });
});

onUnmounted(() => {
  if (unlistenPty) unlistenPty();
  if (statsInterval) clearInterval(statsInterval);
  term?.dispose();
  cpuChart?.dispose();
  memChart?.dispose();
});

const isUnlocking = ref(false);
const shouldShake = ref(false);

const setMasterPass = async () => {
  if (!masterPassword.value) {
    errorMsg.value = 'Please enter a master password to continue.';
    triggerShake();
    return;
  }
  
  isUnlocking.value = true;
  errorMsg.value = '';
  
  try {
    // Artificial slight delay for smoother animation transition
    await new Promise(r => setTimeout(r, 300));
    await invoke('set_master_password', { password: masterPassword.value });
    isMasterPasswordSet.value = true;
    loadServers();
  } catch (e) {
    errorMsg.value = 'Security Error: ' + e;
    triggerShake();
  } finally {
    isUnlocking.value = false;
  }
};

const triggerShake = () => {
  shouldShake.value = true;
  setTimeout(() => shouldShake.value = false, 500);
};

const loadServers = async () => {
  try {
    savedServers.value = await invoke('list_server_configs');
  } catch (e) {
    console.error('Failed to load servers:', e);
  }
};

const saveServer = async () => {
  try {
    const config = {
      id: window.crypto.randomUUID(),
      label: label.value,
      host: host.value,
      user: user.value,
      port: 22,
      password_enc: password.value,
      key_path: null
    };
    await invoke('save_server_config', { config });
    showAddServer.value = false;
    loadServers();
  } catch (e) {
    errorMsg.value = 'Failed to save server: ' + e;
  }
};

const deleteServer = async (id: string) => {
  if (!confirm('Are you sure?')) return;
  try {
    await invoke('delete_server_config', { id });
    loadServers();
  } catch (e) {
    errorMsg.value = 'Failed to delete server: ' + e;
  }
};

const connectWithId = async (id: string) => {
  errorMsg.value = 'Connecting...';
  try {
    await invoke('connect_with_id', { id });
    onConnected();
  } catch (e) {
    errorMsg.value = String(e);
  }
};

const connect = async () => {
  errorMsg.value = 'Connecting...';
  try {
    await invoke('connect_to_ssh', {
      host: host.value,
      user: user.value,
      pass: password.value
    });
    onConnected();
  } catch (e) {
    errorMsg.value = String(e);
  }
};

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  
  setTimeout(() => {
    fitAddon.fit();
    invoke('resize_pty', { cols: term.cols, rows: term.rows });
  }, 100);

  fetchFiles('.');
  term.focus();
  unlistenPty = await listen<number[]>('pty-data', (event) => {
    term.write(new Uint8Array(event.payload));
  });

  statsInterval = window.setInterval(() => {
    fetchStats();
    fetchTasks();
    fetchGuiStatus();
  }, 2000);
  fetchStats();
  fetchTasks();
  fetchGuiStatus();
};

const initAi = async () => {
  if (isAiInitialized.value) return;
  aiLoading.value = true;
  try {
    const engine = await webllm.CreateMLCEngine(MODEL_ID, {
      initProgressCallback: (p) => {
        aiProgress.value = `Loading Model: ${Math.round(p.progress * 100)}% - ${p.text}`;
      }
    });
    aiEngine.value = engine;
    isAiInitialized.value = true;
    aiProgress.value = 'AI Sidekick Ready (Local & Offline)';
  } catch (e) {
    aiProgress.value = 'WebGPU not supported or Model failed: ' + e;
    console.error(e);
  } finally {
    aiLoading.value = false;
  }
};

const sendToAi = async (msg: string = '') => {
  const content = msg || userMessage.value;
  if (!content || !aiEngine.value) return;

  aiChatHistory.value.push({ role: 'user', content });
  userMessage.value = '';
  aiLoading.value = true;
  scrollToBottom();

  try {
    const reply = await aiEngine.value.chat.completions.create({
      messages: aiChatHistory.value as any,
      stream: false
    });
    const assistantMsg = reply.choices[0].message.content || '';
    aiChatHistory.value.push({ role: 'assistant', content: assistantMsg });
    scrollToBottom();
  } catch (e) {
    aiChatHistory.value.push({ role: 'assistant', content: 'Error: ' + e });
  } finally {
    aiLoading.value = false;
  }
};

const explainTerminalError = async () => {
  if (!term) return;
  const lines = [];
  const totalLines = term.buffer.active.length;
  for (let i = Math.max(0, totalLines - 20); i < totalLines; i++) {
    const line = term.buffer.active.getLine(i);
    if (line) lines.push(line.translateToString(true));
  }
  
  const rawOutput = lines.join('\n');
  const sanitized = sanitizeOutput(rawOutput);
  
  showAiPanel.value = true;
  if (!isAiInitialized.value) await initAi();
  
  sendToAi(`Please explain this terminal output and suggest a fix if there is an error. I have removed sensitive info like IPs for security. Output:\n\n${sanitized}`);
};

const explainLogs = async () => {
  const sanitized = sanitizeOutput(selectedTaskLog.value);
  showAiPanel.value = true;
  if (!isAiInitialized.value) await initAi();
  
  sendToAi(`Please analyze these process logs and tell me if there is any issue. I have removed sensitive info like IPs for security. Logs:\n\n${sanitized}`);
};

const sanitizeOutput = (text: string) => {
  return text
    .replace(/\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b/g, '[IP_MASKED]')
    .replace(/(?:password|passwd|secret|key|token)=([^\s&]+)/gi, '$1=[SENSITIVE_MASKED]');
};

const fetchGuiStatus = async () => {
  try {
    const res = await fetch('http://localhost:54321/gui/status', {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      guiStatus.value = await res.json();
    }
  } catch (e) {
    console.error('Failed to fetch GUI status:', e);
  }
};

const initGui = async () => {
  try {
    errorMsg.value = 'Initializing GUI environment (this may take a minute)...';
    const res = await fetch('http://localhost:54321/gui/init', {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      errorMsg.value = 'GUI initialized successfully.';
      setTimeout(() => errorMsg.value = '', 3000);
    }
  } catch (e) {
    alert('Failed to init GUI: ' + e);
  }
};

const fetchTasks = async () => {
  try {
    const res = await fetch('http://localhost:54321/task/list', {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      managedTasks.value = await res.json();
    }
  } catch (e) {
    console.error('Failed to fetch tasks:', e);
  }
};

const startTask = async () => {
  if (!newTaskCmd.value) return;
  const parts = newTaskCmd.value.trim().split(/\s+/);
  const command = parts[0];
  const args = parts.slice(1);
  const id = 'task-' + Date.now();

  try {
    const res = await fetch('http://localhost:54321/task/start', {
      method: 'POST',
      headers: { 
        'Content-Type': 'application/json',
        'X-Ter-Token': agentToken.value 
      },
      body: JSON.stringify({ id, command, args })
    });
    if (res.ok) {
      newTaskCmd.value = '';
      showAddTask.value = false;
      fetchTasks();
    } else {
      alert('Failed to start task: ' + await res.text());
    }
  } catch (e) {
    alert('Error starting task: ' + e);
  }
};

const stopTask = async (id: string) => {
  try {
    const res = await fetch(`http://localhost:54321/task/stop?id=${id}`, {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      fetchTasks();
    }
  } catch (e) {
    console.error('Failed to stop task:', e);
  }
};

const viewLogs = async (id: string) => {
  try {
    const res = await fetch(`http://localhost:54321/task/logs?id=${id}`, {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      selectedTaskLog.value = await res.text();
      showLogModal.value = true;
    }
  } catch (e) {
    console.error('Failed to fetch logs:', e);
  }
};

const fetchStats = async () => {
  try {
    const res = await fetch('http://localhost:54321/stats', {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      stats.value = await res.json();
      updateCharts(stats.value);
    }
  } catch (e) {
    console.error('Failed to fetch stats:', e);
  }
};

const fetchFiles = async (path: string) => {
  try {
    currentPath.value = path;
    fileList.value = await invoke('ls_remote', { path });
  } catch (e) {
    console.error('Failed to fetch files:', e);
  }
};

const handleFileClick = async (f: any) => {
  if (f.is_dir) {
    fetchFiles(currentPath.value + '/' + f.name);
  } else {
    // 使用新的 Dialog 插件保存文件
    const localPath = await save({
      defaultPath: f.name,
    });
    if (localPath) {
      try {
        errorMsg.value = `Downloading ${f.name}...`;
        await invoke('download_file', { 
          remotePath: currentPath.value + '/' + f.name,
          localPath: localPath 
        });
        errorMsg.value = `Downloaded ${f.name} successfully.`;
        setTimeout(() => errorMsg.value = '', 3000);
      } catch (e) {
        alert('Download failed: ' + e);
      }
    }
  }
};

const uploadFile = async () => {
  const selected = await open({
    multiple: false,
    directory: false,
  });
  if (selected && typeof selected === 'string') {
    const fileName = selected.split('/').pop() || 'uploaded_file';
    try {
      errorMsg.value = `Uploading ${fileName}...`;
      await invoke('upload_file', {
        localPath: selected,
        remotePath: currentPath.value + '/' + fileName
      });
      errorMsg.value = `Uploaded ${fileName} successfully.`;
      fetchFiles(currentPath.value); // Refresh list
      setTimeout(() => errorMsg.value = '', 3000);
    } catch (e) {
      alert('Upload failed: ' + e);
    }
  }
};

const killProcess = async (pid: number) => {
  if (!confirm(`Are you sure you want to kill process ${pid}?`)) return;
  try {
    const res = await fetch(`http://localhost:54321/proc/kill?pid=${pid}`, {
      headers: { 'X-Ter-Token': agentToken.value }
    });
    if (res.ok) {
      errorMsg.value = `Process ${pid} killed successfully.`;
      fetchStats(); // Refresh immediately
    } else {
      errorMsg.value = `Failed to kill process: ${await res.text()}`;
    }
  } catch (e) {
    errorMsg.value = `Error killing process: ${e}`;
  }
};

const formatBytes = (bytes: number) => {
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};
const toggleDashboard = () => {
  showDashboard.value = !showDashboard.value;
  if (showDashboard.value) {
    // Next tick
    setTimeout(() => {
      initCharts();
      cpuChart?.resize();
      memChart?.resize();
    }, 100);
  }
};
</script>

<template>
  <div class="app-container">
    <!-- Master Password Setup -->
    <div v-if="!isMasterPasswordSet" class="unlock-overlay">
      <div :class="['unlock-card', { 'shake-anim': shouldShake }]">
        <div class="unlock-icon">🔒</div>
        <h2>Unlock Ter</h2>
        <p>Enter your master password to access secure vault</p>
        
        <div class="unlock-form">
          <div class="input-wrapper">
            <input 
              v-model="masterPassword" 
              type="password" 
              placeholder="Master Password" 
              @keyup.enter="setMasterPass"
              :disabled="isUnlocking"
              autofocus
            />
            <div class="input-focus-bg"></div>
          </div>
          
          <button @click="setMasterPass" :disabled="isUnlocking" class="primary-unlock-btn">
            <span v-if="!isUnlocking" class="btn-content">
              <span class="lock-small">🔒</span> Unlock Vault
            </span>
            <div v-else class="spinner"></div>
          </button>
        </div>

        <div v-if="errorMsg" class="unlock-error-msg">
          <span class="error-icon">⚠️</span> {{ errorMsg }}
        </div>
      </div>
    </div>

    <!-- Login/Server List Panel -->
    <div v-else-if="!isConnected" class="login-panel">
      <h2>Ter: Secure Remote Manager</h2>
      
      <div class="server-management" v-if="!showAddServer">
        <div class="server-list-saved">
          <div v-for="s in savedServers" :key="s.id" class="server-item">
            <div class="server-info-box" @click="connectWithId(s.id)">
              <span class="server-label">{{ s.label }}</span>
              <span class="server-host">{{ s.user }}@{{ s.host }}</span>
            </div>
            <button class="delete-btn" @click="deleteServer(s.id)">✕</button>
          </div>
        </div>
        <button class="add-btn" @click="showAddServer = true">+ Add New Server</button>
      </div>

      <div v-if="showAddServer" class="add-server-form">
        <h3>Add Server</h3>
        <div class="form-group">
          <label>Label:</label>
          <input v-model="label" type="text" />
        </div>
        <div class="form-group">
          <label>Host:</label>
          <input v-model="host" type="text" />
        </div>
        <div class="form-group">
          <label>User:</label>
          <input v-model="user" type="text" />
        </div>
        <div class="form-group">
          <label>Password:</label>
          <input v-model="password" type="password" />
        </div>
        <div class="button-group">
          <button @click="saveServer">Save</button>
          <button @click="connect" class="secondary">Connect Once</button>
          <button @click="showAddServer = false" class="cancel">Cancel</button>
        </div>
      </div>

      <div v-if="errorMsg" class="error">{{ errorMsg }}</div>
    </div>
    
    <!-- Main Interface -->
    <div v-else class="main-layout">
      <!-- 1. Sidebar -->
      <aside :class="['sidebar', { collapsed: !showDashboard }]">
        <div class="sidebar-header">
          <div v-if="showDashboard" class="brand">
            <span class="brand-icon">⚡</span>
            <h3>System</h3>
          </div>
          <button class="toggle-btn" @click="toggleDashboard">
            {{ showDashboard ? '«' : '»' }}
          </button>
        </div>
        
        <div v-if="showDashboard" class="sidebar-scroll">
          <div v-if="stats" class="widget">
            <div class="widget-header">
              <label>CPU Usage</label>
              <span class="value">{{ stats.cpu_usage.toFixed(1) }}%</span>
            </div>
            <div class="chart-box" ref="cpuChartRef"></div>
          </div>
          
          <div v-if="stats" class="widget">
            <div class="widget-header">
              <label>Memory Usage</label>
              <span class="value">{{ formatBytes(stats.mem_used) }}</span>
            </div>
            <div class="chart-box" ref="memChartRef"></div>
          </div>

          <!-- File Explorer Section -->
          <div class="widget explorer">
            <div class="widget-header">
              <label>Remote Files</label>
              <button class="mini-btn" @click="uploadFile">Upload</button>
            </div>
            <div class="current-path">/{{ currentPath }}</div>
            <ul class="file-list">
              <li v-for="f in fileList" :key="f.name" @click="handleFileClick(f)">
                <span :class="['icon', { dir: f.is_dir }]">{{ f.is_dir ? '📁' : '📄' }}</span>
                <span class="name">{{ f.name }}</span>
              </li>
            </ul>
          </div>

          <!-- Managed Tasks -->
          <div class="widget tasks">
            <div class="widget-header">
              <label>Managed Tasks</label>
              <button class="mini-btn" @click="showAddTask = !showAddTask">+</button>
            </div>
            <div v-if="showAddTask" class="inline-input">
              <input v-model="newTaskCmd" placeholder="Command..." @keyup.enter="startTask" />
            </div>
            <ul class="task-list">
              <li v-for="t in managedTasks" :key="t.id">
                <span class="task-name">{{ t.command }}</span>
                <div class="task-ops">
                  <span :class="['dot', t.status]"></span>
                  <button @click="viewLogs(t.id)">Log</button>
                </div>
              </li>
            </ul>
          </div>

          <div class="ai-trigger-card" @click="showAiPanel = true">
            <div class="ai-glow"></div>
            <span class="icon">✨</span>
            <div class="text">
              <label>AI Sidekick</label>
              <small>{{ isAiInitialized ? 'Online' : 'Initialize' }}</small>
            </div>
          </div>
        </div>
      </aside>

      <!-- 2. Main Content Area -->
      <main class="content-area">
        <!-- 2.1 Header / Top Bar -->
        <header class="top-bar">
          <div class="connection-status">
            <span class="status-led online"></span>
            <span class="connection-info">{{ user }}@{{ host }}</span>
          </div>
          <div class="actions">
            <button @click="explainTerminalError" class="ai-btn">
              <span class="star">✦</span> Explain Output
            </button>
            <button @click="showGui = !showGui" class="gui-btn">
              🖥️ Remote Desktop
            </button>
          </div>
        </header>

        <!-- 2.2 Terminal Workspace -->
        <div class="terminal-workspace">
          <div class="terminal-card">
            <div class="terminal-container" ref="terminalRef"></div>
          </div>
        </div>
      </main>

      <!-- AI Side Panel (Overlay/Slide-in) -->
      <Transition name="slide">
        <div v-if="showAiPanel" class="ai-panel">
          <div class="panel-header">
            <h3>AI Diagnosis</h3>
            <button @click="showAiPanel = false">✕</button>
          </div>
          <!-- AI Content remains same but styled better -->
          <div v-if="!isAiInitialized" class="ai-init">
            <p>Loading local intelligence...</p>
            <div class="p-bar"><div class="p-fill" :style="{width: aiProgress.includes('%') ? aiProgress.match(/\d+/)?.[0]+'%' : '5%'}"></div></div>
            <button @click="initAi">Initialize WebGPU AI</button>
          </div>
          <div v-else class="chat-flow">
            <div class="messages" ref="chatRef">
              <div v-for="(msg, i) in aiChatHistory" :key="i" :class="['bubble', msg.role]">
                {{ msg.content }}
              </div>
            </div>
            <div class="chat-input-box">
              <input v-model="userMessage" placeholder="Ask about output..." @keyup.enter="sendToAi()" />
            </div>
          </div>
        </div>
      </Transition>
    </div>
  </div>
</template>

<style scoped>
/* Color Palette */
:root {
  --bg-dark: #09090b;
  --bg-sidebar: #121215;
  --bg-card: #18181b;
  --accent: #6366f1;
  --text-primary: #fafafa;
  --text-muted: #a1a1aa;
  --border: #27272a;
}

.app-container {
  height: 100vh;
  width: 100vw;
  background-color: #09090b;
  color: #fafafa;
  font-family: 'Inter', system-ui, sans-serif;
  overflow: hidden;
}

/* Three-Pane Layout */
.main-layout {
  display: flex;
  height: 100%;
  width: 100%;
}

/* Sidebar Styling */
.sidebar {
  width: 280px;
  background: #121215;
  border-right: 1px solid #27272a;
  display: flex;
  flex-direction: column;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.sidebar.collapsed {
  width: 60px;
}

.sidebar-header {
  padding: 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #27272a;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.brand h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  letter-spacing: -0.02em;
}

.sidebar-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.widget {
  background: #18181b;
  border: 1px solid #27272a;
  border-radius: 12px;
  padding: 12px;
}

.widget-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.widget-header label {
  font-size: 11px;
  text-transform: uppercase;
  color: #71717a;
  letter-spacing: 0.05em;
  font-weight: 600;
}

.chart-box {
  height: 80px;
  width: 100%;
}

/* Content Area */
.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #09090b;
  position: relative;
}

/* Top Bar */
.top-bar {
  height: 56px;
  background: #121215;
  border-bottom: 1px solid #27272a;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  z-index: 10;
}

.connection-status {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-led {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #22c55e;
  box-shadow: 0 0 12px rgba(34, 197, 94, 0.4);
}

.connection-info {
  font-family: 'JetBrains Mono', monospace;
  font-size: 13px;
  color: #a1a1aa;
}

/* AI Button Styling */
.ai-btn {
  background: linear-gradient(135deg, #6366f1 0%, #a855f7 100%);
  border: none;
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 600;
  color: white;
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  transition: transform 0.2s;
}

.ai-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
}

/* Terminal Workspace */
.terminal-workspace {
  flex: 1;
  padding: 24px;
  display: flex;
  justify-content: center;
  align-items: center;
}

.terminal-card {
  width: 100%;
  height: 100%;
  background: #000;
  border: 1px solid #27272a;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
  display: flex;
  position: relative;
}

/* Terminal text padding */
.terminal-container {
  flex: 1;
  padding: 16px; /* 增加终端内边距 */
  background: #000;
}

/* AI Sidebar Panel */
.ai-panel {
  position: absolute;
  top: 70px;
  right: 20px;
  bottom: 20px;
  width: 380px;
  background: rgba(18, 18, 21, 0.85);
  backdrop-filter: blur(12px);
  border: 1px solid #3f3f46;
  border-radius: 16px;
  box-shadow: -20px 0 50px rgba(0,0,0,0.5);
  display: flex;
  flex-direction: column;
  z-index: 100;
}

/* Transitions */
.slide-enter-active, .slide-leave-active {
  transition: all 0.3s ease;
}
.slide-enter-from, .slide-leave-to {
  transform: translateX(400px);
  opacity: 0;
}

/* Widget Lists */
.file-list, .task-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.file-list li {
  padding: 6px 8px;
  border-radius: 6px;
  font-size: 12px;
  display: flex;
  gap: 10px;
  cursor: pointer;
}

.file-list li:hover {
  background: #27272a;
}

.ai-trigger-card {
  position: relative;
  background: #1e1e2e;
  border: 1px solid #313244;
  border-radius: 12px;
  padding: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  overflow: hidden;
}

.ai-glow {
  position: absolute;
  top: -20px;
  left: -20px;
  width: 100px;
  height: 100px;
  background: radial-gradient(circle, rgba(99, 102, 241, 0.2) 0%, transparent 70%);
}

/* Modal/Overlay Styles (Unchanged but adapted) */
.unlock-overlay {
  position: fixed;
  inset: 0;
  background: #09090b;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.unlock-card {
  background: #18181b;
  border: 1px solid #27272a;
  padding: 40px;
  border-radius: 20px;
  width: 400px;
  text-align: center;
}

.unlock-card input {
  width: 100%;
  padding: 14px;
  background: #09090b;
  border: 1px solid #27272a;
  border-radius: 10px;
  color: white;
  margin-bottom: 20px;
}

button.primary-unlock-btn {
  width: 100%;
  padding: 14px;
  background: #6366f1;
  border-radius: 10px;
  font-weight: bold;
}
</style>

      <!-- Remote Desktop Overlay -->
      <div v-if="showGui" class="gui-overlay">
        <div class="gui-header">
          <h3>Remote Desktop (VNC via SSH)</h3>
          <div class="gui-controls">
            <small>Tunnel: localhost:55901 -> :5901</small>
            <button @click="showGui = false" class="close-btn">✕</button>
          </div>
        </div>
        <div class="gui-body">
          <div v-if="!guiStatus.running" class="gui-placeholder">
            <p>Please start the GUI environment from the sidebar first.</p>
          </div>
          <div v-else class="vnc-container">
            <p>VNC Server Active on :1 (Port 5901)</p>
            <p>SSH Tunnel established: <b>localhost:55901</b></p>
            <div class="vnc-box">
              <p>In this prototype, you can use any local VNC viewer to connect to <code>localhost:55901</code>.</p>
              <p>The display is currently running <b>Fluxbox</b> (Minimal Window Manager).</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Log Modal -->
      <div v-if="showLogModal" class="modal-overlay" @click.self="showLogModal = false">
        <div class="modal-content">
          <div class="modal-header">
            <h3>Task Logs</h3>
            <div class="modal-tools">
              <button @click="explainLogs" class="tool-btn">✨ AI Explain</button>
              <button @click="showLogModal = false">✕</button>
            </div>
          </div>
          <pre class="log-viewer">{{ selectedTaskLog }}</pre>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  background-color: #1e1e1e;
  color: #fff;
  overflow: hidden;
}

/* Unlock and Login Overhaul */
.unlock-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: radial-gradient(circle at center, #1e1e1e 0%, #0a0a0a 100%);
  z-index: 2000;
}

.unlock-card {
  width: 380px;
  padding: 40px;
  background: #181818;
  border: 1px solid #333;
  border-radius: 16px;
  text-align: center;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
}

.unlock-icon {
  font-size: 48px;
  margin-bottom: 20px;
}

.unlock-card h2 {
  margin: 0 0 10px 0;
  font-size: 24px;
  font-weight: 700;
  color: #fff;
}

.unlock-card p {
  color: #888;
  font-size: 14px;
  margin-bottom: 30px;
}

.unlock-form {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.input-wrapper {
  position: relative;
  width: 100%;
}

.unlock-card input {
  width: 100%;
  padding: 14px 16px;
  background: #252526;
  border: 1px solid #444;
  border-radius: 8px;
  color: #fff;
  font-size: 16px;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  box-sizing: border-box;
}

.unlock-card input:focus {
  outline: none;
  border-color: #007acc;
  box-shadow: 0 0 0 4px rgba(0, 122, 204, 0.15);
  background: #2d2d2d;
}

.primary-unlock-btn {
  width: 100%;
  padding: 14px;
  background: linear-gradient(135deg, #007acc 0%, #005a9e 100%);
  color: #fff;
  border: none;
  border-radius: 8px;
  font-size: 15px;
  font-weight: 600;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 48px;
}

.primary-unlock-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 4px 15px rgba(0, 122, 204, 0.4);
}

.primary-unlock-btn:active:not(:disabled) {
  transform: translateY(0);
}

.primary-unlock-btn:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.btn-content {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Animations */
.shake-anim {
  animation: shake 0.5s cubic-bezier(.36,.07,.19,.97) both;
}

@keyframes shake {
  10%, 90% { transform: translate3d(-1px, 0, 0); }
  20%, 80% { transform: translate3d(2px, 0, 0); }
  30%, 50%, 70% { transform: translate3d(-4px, 0, 0); }
  40%, 60% { transform: translate3d(4px, 0, 0); }
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid rgba(255,255,255,0.3);
  border-radius: 50%;
  border-top-color: #fff;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.unlock-error-msg {
  margin-top: 20px;
  padding: 10px;
  background: rgba(255, 82, 82, 0.1);
  border-radius: 6px;
  color: #ff5252;
  font-size: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}

/* Reusing some styles for login-panel but centering it too */
.login-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  background: radial-gradient(circle at center, #1e1e1e 0%, #0a0a0a 100%);
  gap: 20px;
}

.server-management {
  width: 400px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.server-list-saved {
  max-height: 400px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.server-item {
  display: flex;
  background: #333;
  border-radius: 6px;
  overflow: hidden;
  align-items: stretch;
  border: 1px solid #444;
}

.server-info-box {
  flex: 1;
  padding: 12px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.server-info-box:hover {
  background: #3c3c3c;
}

.server-label {
  font-weight: bold;
  font-size: 14px;
}

.server-host {
  font-size: 12px;
  color: #aaa;
}

.delete-btn {
  background: transparent;
  color: #ff5252;
  border: none;
  width: 40px;
  font-size: 16px;
}

.delete-btn:hover {
  background: rgba(255, 82, 82, 0.1);
}

.add-btn {
  background: #333;
  border: 1px dashed #555;
  color: #aaa;
  padding: 10px;
  border-radius: 6px;
}

.add-btn:hover {
  border-color: #007acc;
  color: #007acc;
}

.add-server-form {
  width: 350px;
  background: #252526;
  padding: 20px;
  border-radius: 8px;
  border: 1px solid #333;
}

.button-group {
  display: flex;
  gap: 10px;
  margin-top: 20px;
}

.button-group button {
  flex: 1;
  padding: 8px;
}

button.secondary {
  background: #333;
  border: 1px solid #444;
}

button.cancel {
  background: transparent;
  color: #aaa;
}

.form-group {
  display: flex;
  flex-direction: column;
  width: 300px;
}

.form-group input {
  padding: 8px;
  border-radius: 4px;
  border: 1px solid #444;
  background: #333;
  color: white;
}

.main-layout {
  display: flex;
  flex: 1;
  height: 100%;
}

.dashboard-sidebar {
  width: 260px;
  background: #252526;
  border-right: 1px solid #333;
  display: flex;
  flex-direction: column;
  transition: width 0.3s;
}

.dashboard-sidebar.collapsed {
  width: 40px;
}

.sidebar-header {
  padding: 10px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #333;
  white-space: nowrap;
}

.stats-content {
  padding: 15px;
  overflow-y: auto;
}

.stat-card {
  margin-bottom: 20px;
}

.chart-container {
  height: 120px;
  width: 100%;
}

.stat-card label {
  display: block;
  font-size: 12px;
  color: #aaa;
  margin-bottom: 5px;
}

.progress-bar {
  height: 8px;
  background: #444;
  border-radius: 4px;
  overflow: hidden;
  margin-bottom: 5px;
}

.progress {
  height: 100%;
  background: #007acc;
}

.file-explorer {
  margin-top: 20px;
  border-top: 1px solid #444;
  padding-top: 10px;
  margin-bottom: 20px;
}

.explorer-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.upload-btn {
  padding: 2px 6px;
  font-size: 10px;
  background: #333;
  border: 1px solid #555;
  border-radius: 3px;
}

.file-list {
  list-style: none;
  padding: 0;
  font-size: 11px;
  max-height: 250px;
  overflow-y: auto;
}

.file-list li {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 0;
  cursor: pointer;
  user-select: none;
  position: relative;
}

.file-list li:hover {
  background: #333;
}

.file-icon {
  font-size: 14px;
}

.file-icon.dir {
  color: #ffca28;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.download-hint {
  font-size: 10px;
  opacity: 0;
  transition: opacity 0.2s;
}

.file-list li:hover .download-hint {
  opacity: 0.6;
}

.proc-list {
  list-style: none;
  padding: 0;
  font-size: 11px;
}

.proc-list li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 0;
  border-bottom: 1px solid #333;
}

.proc-info {
  display: flex;
  justify-content: space-between;
  flex: 1;
  margin-right: 10px;
}

.proc-name {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.proc-cpu {
  color: #4caf50;
  font-weight: bold;
}

.kill-btn {
  background: transparent;
  color: #ff5252;
  border: 1px solid #444;
  padding: 2px 6px;
  border-radius: 3px;
  cursor: pointer;
  font-size: 10px;
  line-height: 1;
}

.kill-btn:hover {
  background: #ff5252;
  color: white;
  border-color: #ff5252;
}

.managed-tasks {
  margin-top: 20px;
  border-top: 1px solid #444;
  padding-top: 10px;
}

.add-task-inline {
  margin-bottom: 8px;
}

.add-task-inline input {
  width: 100%;
  padding: 4px 8px;
  background: #333;
  border: 1px solid #555;
  border-radius: 3px;
  color: white;
  font-size: 11px;
}

.task-list {
  list-style: none;
  padding: 0;
  font-size: 11px;
}

.task-list li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid #333;
}

.task-info {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
}

.task-cmd {
  font-weight: bold;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-status {
  font-size: 10px;
  color: #aaa;
}

.task-status.running { color: #4caf50; }
.task-status.stopped { color: #f44336; }
.task-status.failed { color: #ff9800; }

.task-actions {
  display: flex;
  gap: 4px;
}

.task-actions button {
  padding: 2px 4px;
  background: transparent;
  border: 1px solid #444;
  border-radius: 3px;
  font-size: 10px;
}

/* Modal Styles */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: #252526;
  width: 80%;
  height: 80%;
  display: flex;
  flex-direction: column;
  border-radius: 8px;
  border: 1px solid #444;
  overflow: hidden;
}

.modal-header {
  padding: 10px 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #333;
}

.log-viewer {
  flex: 1;
  padding: 20px;
  background: #1e1e1e;
  color: #ccc;
  overflow-y: auto;
  font-family: 'Menlo', monospace;
  font-size: 12px;
  margin: 0;
  white-space: pre-wrap;
}

.gui-status-info {
  margin-top: 5px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.status-hint {
  font-size: 10px;
  color: #ff9800;
}

.status-hint.running {
  color: #4caf50;
}

.init-btn {
  padding: 4px 8px;
  background: #333;
  border: 1px solid #555;
  border-radius: 4px;
  font-size: 10px;
  color: white;
}

/* GUI Overlay Styles */
.gui-overlay {
  position: fixed;
  top: 50px;
  left: 280px;
  right: 20px;
  bottom: 20px;
  background: #252526;
  border: 1px solid #444;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  z-index: 900;
  box-shadow: 0 10px 30px rgba(0,0,0,0.5);
}

.gui-header {
  padding: 10px 15px;
  background: #333;
  border-bottom: 1px solid #444;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-radius: 8px 8px 0 0;
}

.gui-controls {
  display: flex;
  align-items: center;
  gap: 15px;
}

.gui-controls small {
  color: #4caf50;
  font-family: monospace;
}

.gui-body {
  flex: 1;
  background: #000;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #aaa;
}

.vnc-container {
  text-align: center;
}

.vnc-box {
  margin-top: 20px;
  padding: 20px;
  background: #1e1e1e;
  border: 1px dashed #555;
  border-radius: 8px;
}

.vnc-box code {
  color: #ffca28;
  font-size: 1.2em;
}

.close-btn {
  background: transparent;
  border: none;
  color: white;
  font-size: 16px;
  cursor: pointer;
}

.status-msg {
  font-size: 10px;
  color: #4caf50;
  margin-top: 10px;
}

.terminal-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #1e1e1e;
}

.terminal-toolbar {
  padding: 5px 10px;
  background: #252526;
  border-bottom: 1px solid #333;
  display: flex;
  justify-content: flex-end;
}

.tool-btn {
  background: #333;
  color: #ccc;
  border: 1px solid #444;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
}

.tool-btn:hover {
  border-color: #007acc;
  color: white;
}

/* AI Panel Styles */
.ai-entry-card {
  margin-top: auto;
  padding: 10px;
  background: #333;
  border: 1px solid #444;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  transition: all 0.2s;
}

.ai-entry-card:hover {
  border-color: #007acc;
  background: #3c3c3c;
}

.ai-icon {
  font-size: 20px;
}

.ai-text label {
  display: block;
  font-size: 12px;
  font-weight: bold;
}

.ai-text small {
  font-size: 10px;
  color: #4caf50;
}

.ai-side-panel {
  width: 350px;
  background: #252526;
  border-left: 1px solid #333;
  display: flex;
  flex-direction: column;
}

.ai-header {
  padding: 10px 15px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #333;
}

.ai-init-screen {
  padding: 20px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  gap: 15px;
}

.ai-progress-bar {
  width: 100%;
  height: 6px;
  background: #333;
  border-radius: 3px;
  overflow: hidden;
}

.ai-progress-fill {
  height: 100%;
  background: #007acc;
  transition: width 0.3s;
}

.progress-text {
  font-size: 11px;
  color: #aaa;
}

.ai-chat-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 15px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.chat-msg {
  max-width: 90%;
  display: flex;
}

.chat-msg.user {
  align-self: flex-end;
}

.chat-msg.assistant {
  align-self: flex-start;
}

.msg-bubble {
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.4;
  white-space: pre-wrap;
}

.chat-msg.user .msg-bubble {
  background: #007acc;
  color: white;
}

.chat-msg.assistant .msg-bubble {
  background: #333;
  color: #ccc;
  border: 1px solid #444;
}

.chat-input {
  padding: 10px;
  background: #1e1e1e;
  border-top: 1px solid #333;
  display: flex;
  gap: 8px;
}

.chat-input input {
  flex: 1;
  padding: 8px;
  background: #333;
  border: 1px solid #444;
  border-radius: 4px;
  color: white;
}

.modal-tools {
  display: flex;
  gap: 10px;
}

.terminal-container {
  flex: 1;
  background: #1e1e1e;
}

button {
  padding: 10px 20px;
  background: #007acc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.toggle-btn {
  padding: 2px 8px;
  background: transparent;
  border: 1px solid #444;
}

.error {
  color: #ff5252;
  font-size: 12px;
}
</style>
