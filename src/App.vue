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
const previewUrl = ref('http://localhost:5173');
const backendLogs = ref<string[]>([]);
const logsContainerRef = ref<HTMLElement | null>(null);
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

// Persistence Logic
const storageKey = computed(() => `ter_tabs_${host.value.replace(/\s+/g, '_')}`);

// 1. Save tabs to localStorage on change
watch(terminalTabs, (newTabs) => {
  if (isConnected.value) {
    localStorage.setItem(storageKey.value, JSON.stringify(newTabs));
  }
}, { deep: true });

// SFTP / Data State
const realFiles = ref<any[]>([]);
const skills = ref<any[]>([]);

// ==========================================
// --- TAB MANAGEMENT ---
// ==========================================
const createNewTab = async (title = "Shell", skipPty = false, existingId?: string) => {
  const id = existingId || 'tab-' + Math.random().toString(36).substr(2, 9);
  
  // 1. Setup local Terminal instance
  terminalManager.setOnDataCallback(id, (tid, data) => {
    if (!skipPty && isConnected.value) invoke('write_pty', { tabId: tid, data });
  });
  terminalManager.getOrCreate(id);
  
  // 2. Spawn remote PTY if connected and not in skipPty mode
  if (!skipPty && isConnected.value) {
    try {
      await invoke('spawn_new_pty', { tabId: id });
    } catch (e) {
      console.error("Failed to spawn remote PTY:", e);
      backendLogs.value.push(`[ERROR] Failed to spawn PTY: ${e}`);
    }
  }

  if (!existingId) {
    terminalTabs.value.push({ id, title, isBackground: false });
  }
  activeTabId.value = id;
  return id;
};

const viewHistory = async (originalTabId: string) => {
  const originalTab = terminalTabs.value.find(t => t.id === originalTabId);
  const title = `Playback: ${originalTab?.title || originalTabId}`;
  
  const playbackId = await createNewTab(title, true); // skipPty = true
  
  try {
    const logs = await invoke<number[][]>('get_terminal_logs', { tabId: originalTabId, limit: 1000 });
    backendLogs.value.push(`[INFO] Replaying ${logs.length} chunks from history...`);
    
    // Playback with slight delay for "Movie" effect
    for (const chunk of logs) {
      const bytes = new Uint8Array(chunk);
      terminalManager.write(playbackId, bytes);
      await new Promise(r => setTimeout(r, 20)); // 20ms delay between chunks
    }
  } catch (e) {
    console.error("Failed to load history:", e);
    terminalManager.write(playbackId, `\r\n[ERROR] Failed to load history: ${e}\r\n`);
  }
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
  if (selection) {
    await navigator.clipboard.writeText(selection);
    backendLogs.value.push(`[INFO] Copied ${selection.length} chars to clipboard.`);
  }
  showContextMenu.value = false;
};

const pasteFromClipboard = async () => {
  const id = contextMenuTabId.value || activeTabId.value;
  if (!id) return;
  try {
    const text = await navigator.clipboard.readText();
    if (text) {
      invoke('write_pty', { tabId: id, data: text });
    }
  } catch (e) {
    console.error("Paste failed:", e);
  }
  showContextMenu.value = false;
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
  
  // 1. Restore from localStorage or create new
  const saved = localStorage.getItem(storageKey.value);
  if (saved) {
    try {
      const tabs = JSON.parse(saved);
      terminalTabs.value = tabs;
      backendLogs.value.push(`[INFO] Restoring ${tabs.length} tabs...`);
      for (const t of tabs) {
        // Re-init PTY for existing tab IDs
        await createNewTab(t.title, false, t.id);
      }
      activeTabId.value = tabs.find((t: any) => !t.isBackground)?.id || tabs[0]?.id;
    } catch (e) {
      console.error("Failed to restore tabs:", e);
      await createNewTab("Main Shell");
    }
  } else {
    await createNewTab("Main Shell");
  }

  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<any>('pty-data', (event) => {
    const { id, data } = event.payload;
    const bytes = new Uint8Array(data);
    const text = new TextDecoder().decode(bytes);

    // [Auto-Pilot]: 自动检测并同意 Gemini CLI 的 Action Required 弹窗
    if (isAutoPilot.value && id === activeTabId.value) {
      // 去除 ANSI 控制字符以便于精准匹配纯文本
      const plainText = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
      if (plainText.includes('Allow execution of:') || plainText.includes('1. Allow once')) {
        console.log("[Auto-Pilot] Detected Action Required. Auto-approving...");
        const randomDelay = Math.floor(Math.random() * 301) + 200;
        setTimeout(() => {
          invoke('write_pty', { tabId: id, data: "\r" });
        }, randomDelay);
      }
    }

    // [反向控制]: 拦截来自 AI 的 [TER_RPC] 指令 (仅对活跃 Tab 有效，避免干扰)
    if (text.includes('[TER_RPC]') && id === activeTabId.value) {
      const rpcRegex = /\[TER_RPC\]\s*({.*?})/g;
      let match;
      let cleanedText = text;
      let foundRpc = false;

      while ((match = rpcRegex.exec(text)) !== null) {
        if (!match[1]) continue;
        try {
          const rpc = JSON.parse(match[1]);
          console.log("[RPC] Intercepted from AI:", rpc);
          foundRpc = true;

          if (rpc.action === 'screenshot') {
            captureAndUpload();
          } else if (rpc.action === 'notify') {
            backendLogs.value.push(`[🔔 NOTIFY] ${rpc.msg || rpc.message || 'New message from AI'}`);
          } else if (rpc.action === 'chart') {
            backendLogs.value.push(`[📊 AI CHART DATA] ${JSON.stringify(rpc.data)}`);
          } else if (rpc.action === 'read_tab') {
            const target = rpc.target || activeTabId.value;
            const lines = rpc.lines || 50;
            const buffer = terminalManager.getBufferText(target, lines);
            invoke('write_pty', { tabId: id, data: `\n[TER_CONTEXT_START: ${target}]\n${buffer}\n[TER_CONTEXT_END]\n` });
          } else if (rpc.action === 'get_cwd_files') {
            const filesData = JSON.stringify(realFiles.value);
            invoke('write_pty', { tabId: id, data: `\n[TER_FILES_START]\n${filesData}\n[TER_FILES_END]\n` });
          }
          
          cleanedText = cleanedText.replace(match[0], '');
        } catch (e) {
          console.warn("RPC Parse Error:", e);
        }
      }

      if (foundRpc) {
        if (cleanedText.trim() === '') return; // Fully consumed
        terminalManager.write(id, new TextEncoder().encode(cleanedText));
        return;
      }
    }

    terminalManager.write(id, bytes);
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

const captureAndUpload = async () => {
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
    await invoke<string>('upload_ui_snapshot', { base64Data });
    const lastLogs = backendLogs.value.slice(-10).join('\n');
    await invoke('write_remote_text', { text: lastLogs, remotePath: '/tmp/current_logs.json' });

    const prompt = ` @../../../../../tmp/current_ui.png 请作为前端专家，看一眼这张刚刚截取的系统UI图。有没有什么明显的错位、报错或者需要优化的地方？`;
    const payload = `\x1b[200~${prompt}\x1b[201~\r`;
    if (activeTabId.value) {
      await invoke('write_pty', { tabId: activeTabId.value, data: payload });
    }
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
      await captureAndUpload();
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
    const cleanRpc = rpc.trim();
    const payload = `\x1b[200~${cleanRpc}\x1b[201~\r`;
    if (activeTabId.value) {
      invoke('write_pty', { tabId: activeTabId.value, data: payload });
    }
  }
};

const refreshWebview = async () => {
  const urlStr = previewUrl.value.trim();
  if (!urlStr) return;

  const match = urlStr.match(/(?:localhost|127\.0\.0\.1):(\d+)/);
  if (match && match[1]) {
    const remotePort = parseInt(match[1]);
    try {
      backendLogs.value.push(`[INFO] Requesting tunnel for port ${remotePort}...`);
      const localPort = await invoke<number>('open_dynamic_tunnel', { remotePort });
      previewUrl.value = `http://localhost:${localPort}`;
      backendLogs.value.push(`[INFO] Tunnel established: ${previewUrl.value}`);
    } catch (e) {
      console.error("Failed to open dynamic tunnel:", e);
      backendLogs.value.push(`[ERROR] Tunnel failed: ${e}`);
    }
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
    if (backendLogs.value.length > 500) backendLogs.value.shift();
    nextTick(() => {
      if (logsContainerRef.value) {
        logsContainerRef.value.scrollTop = logsContainerRef.value.scrollHeight;
      }
    });
  });  window.addEventListener('keydown', (e) => { 
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
        :currentPath="currentPath"
        :bgTabs="backgroundTabs"
        :skills="skills"
        :cpuChartRef="(el: any) => cpuChartRef = el"
        :memChartRef="(el: any) => memChartRef = el"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground"
        @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory"
        @proc-context="onProcContext"
        @run-skill="runSkill"
        @change-dir="changeDir"
      />

      <main class="workspace" ref="workspaceRef" @click="activeTabId && terminalManager.focus(activeTabId)">
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <div class="menu-item" @click="sendToBackground">🚀 Background Task</div>
          <div class="menu-divider"></div>
          <div class="menu-item" :class="{ disabled: !contextMenuTabId || !terminalManager.hasSelection(contextMenuTabId) }" @click="copySelectedText">📋 Copy Selection</div>
          <div class="menu-item" @click="pasteFromClipboard">📥 Paste Clipboard</div>
        </div>

        <nav class="tool-bar">
          <div class="status-chip"><span class="pulse purple"></span> {{ host }}</div>
          <div class="actions">
            <button @click="isLocked = true" class="btn-tool">Lock System</button>
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
                <div class="logs-container" ref="logsContainerRef">
                  <div v-for="(log, i) in backendLogs" :key="i" class="log-line">
                    <span class="line-num">{{ i + 1 }}</span> {{ log }}
                  </div>
                </div>
              </div>
              <div class="cyber-divider"></div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <div class="address-input-wrapper">
                    <span class="secure-icon">🔒</span>
                    <input 
                      v-model="previewUrl" 
                      @keyup.enter="refreshWebview" 
                      class="address-bar-input" 
                      placeholder="Enter remote URL (e.g. localhost:3000)"
                    />
                  </div>
                  <button class="refresh-btn" @click="refreshWebview">⚡</button>
                </nav>
                <CyberWebview ref="webviewRef" :url="previewUrl" />
              </div>
            </div>
          </section>
        </div>

        <!-- NEW: Status Bar -->
        <footer class="status-bar">
          <div class="status-left">
            <span class="item">🟢 {{ host }}</span>
            <span class="item separator">|</span>
            <span class="item">Agent: {{ currentAgentPort ? 'Active' : 'Offline' }}</span>
          </div>
          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload">📸 Audit UI</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">
              {{ cyberMode === 1 ? '🖥️ Terminal Focus' : '🌐 Cyber View' }}
            </button>
            <div class="status-toggle">
              <span>Auto-Pilot</span>
              <label class="mini-switch">
                <input type="checkbox" v-model="isAutoPilot" />
                <span class="slider"></span>
              </label>
            </div>
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

/* Status Bar */
.status-bar { 
  height: 24px; 
  background: #007acc; 
  color: #fff; 
  display: flex; 
  justify-content: space-between; 
  align-items: center; 
  padding: 0 10px; 
  font-size: 11px; 
  z-index: 100;
  flex-shrink: 0;
}

/* Alternative Status Bar color for non-focused mode if you prefer */
.status-bar { background: #18181b; border-top: 1px solid #27272a; color: #71717a; }

.status-left, .status-right { display: flex; align-items: center; gap: 15px; }
.status-left .item { display: flex; align-items: center; }
.status-left .separator { opacity: 0.3; }

.status-btn { 
  background: transparent; 
  border: none; 
  color: #a1a1aa; 
  cursor: pointer; 
  font-size: 11px; 
  padding: 2px 6px;
  border-radius: 4px;
  transition: all 0.2s;
}
.status-btn:hover { background: rgba(255, 255, 255, 0.08); color: #fff; }

.status-toggle { display: flex; align-items: center; gap: 8px; font-size: 11px; color: #71717a; }

/* Mini Switch for Status Bar */
.mini-switch { position: relative; display: inline-block; width: 24px; height: 12px; }
.mini-switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #3f3f46; transition: .4s; border-radius: 12px; }
.slider:before { position: absolute; content: ""; height: 8px; width: 8px; left: 2px; bottom: 2px; background-color: white; transition: .4s; border-radius: 50%; }
input:checked + .slider { background-color: #3b82f6; }
input:checked + .slider:before { transform: translateX(12px); }

.workspace-body { flex: 1; display: flex; overflow: hidden; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; position: relative; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; background: #09090b; overflow: hidden; }
.cyber-container { display: flex; flex-direction: column; height: 100%; }
.cyber-logs-view { flex: 0 0 35%; display: flex; flex-direction: column; background: #09090b; border-bottom: 1px solid #27272a; overflow: hidden; }
.cyber-logs-view header { padding: 8px 12px; background: #09090b; border-bottom: 1px solid #27272a; }
.cyber-logs-view .title { font-size: 10px; color: #3b82f6; font-weight: bold; text-transform: uppercase; letter-spacing: 0.05em; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; scroll-behavior: smooth; }
.log-line { margin-bottom: 2px; white-space: pre-wrap; word-break: break-all; opacity: 0.8; }
.line-num { color: #3f3f46; margin-right: 8px; }
.cyber-divider { height: 1px; background: #27272a; }
.cyber-webview-wrapper { flex: 1; background: #09090b; position: relative; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { height: 32px; background: #09090b; border-bottom: 1px solid #27272a; display: flex; align-items: center; padding: 0 8px; gap: 8px; flex-shrink: 0; }
.address-input-wrapper { flex: 1; background: #18181b; border: 1px solid #27272a; border-radius: 6px; display: flex; align-items: center; padding: 0 8px; height: 24px; }
.secure-icon { font-size: 10px; opacity: 0.5; margin-right: 6px; }
.address-bar-input { background: transparent; border: none; color: #a1a1aa; font-size: 10px; width: 100%; outline: none; font-family: 'JetBrains Mono', monospace; }
.refresh-btn { background: transparent; border: none; color: #3b82f6; cursor: pointer; font-size: 12px; padding: 2px 4px; border-radius: 4px; display: flex; align-items: center; justify-content: center; }
.refresh-btn:hover { background: rgba(59, 130, 246, 0.1); }
.context-menu { position: fixed; z-index: 100000; background: #18181b; border: 1px solid #3f3f46; border-radius: 6px; padding: 4px; min-width: 150px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; border-radius: 4px; }
.menu-item:hover { background: #3b82f6; color: #fff; }
.menu-item.disabled { color: #52525b; cursor: not-allowed; }
.menu-divider { height: 1px; background: #27272a; margin: 4px 0; }
.status-chip { font-size: 11px; color: #a1a1aa; display: flex; align-items: center; font-family: 'JetBrains Mono', monospace; }
.pulse { width: 6px; height: 6px; background: #3b82f6; border-radius: 50%; margin-right: 8px; box-shadow: 0 0 8px #3b82f6; }
.btn-tool { background: transparent; border: 1px solid #27272a; color: #71717a; padding: 3px 10px; border-radius: 4px; cursor: pointer; font-size: 10px; margin-left: 10px; text-transform: uppercase; letter-spacing: 0.05em; }
.btn-tool:hover { border-color: #3b82f6; color: #fff; }
.modal-overlay { position: fixed; inset: 0; background: rgba(9, 9, 11, 0.9); display: flex; align-items: center; justify-content: center; z-index: 10000; backdrop-filter: blur(4px); }
.auth-card { background: #18181b; padding: 30px; border-radius: 12px; border: 1px solid #27272a; width: 320px; box-shadow: 0 20px 50px rgba(0,0,0,0.5); }
.auth-card h2 { font-size: 18px; margin-bottom: 20px; color: #fff; text-align: center; }
.auth-card input { width: 100%; padding: 12px; background: #09090b; border: 1px solid #27272a; color: #fff; border-radius: 6px; margin-bottom: 15px; outline: none; }
.auth-card input:focus { border-color: #3b82f6; }
.btn-primary { width: 100%; padding: 12px; background: #3b82f6; border: none; color: #fff; border-radius: 6px; cursor: pointer; font-weight: bold; transition: background 0.2s; }
.btn-primary:hover { background: #2563eb; }
.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; background: #09090b; }
.vault-container { width: 480px; background: #18181b; border: 1px solid #27272a; border-radius: 12px; padding: 30px; box-shadow: 0 20px 50px rgba(0,0,0,0.5); }
.vault-container header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 25px; }
.vault-container h3 { font-size: 16px; color: #a1a1aa; letter-spacing: 0.05em; text-transform: uppercase; }
.btn-add { background: #27272a; border: none; color: #fff; width: 24px; height: 24px; border-radius: 4px; cursor: pointer; font-size: 18px; line-height: 1; }
.btn-add:hover { background: #3f3f46; }
.server-list { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.server-card { background: #09090b; border: 1px solid #27272a; padding: 15px; border-radius: 10px; display: flex; align-items: center; cursor: pointer; transition: all 0.2s; gap: 12px; }
.server-card:hover { border-color: #3b82f6; background: rgba(59, 130, 246, 0.05); }
.icon-box { background: #27272a; padding: 6px 8px; border-radius: 6px; font-size: 10px; font-weight: bold; color: #a1a1aa; }
.info b { display: block; font-size: 13px; color: #e4e4e7; margin-bottom: 2px; }
.info small { color: #52525b; font-size: 11px; }
</style>
