<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as echarts from 'echarts';
import html2canvas from 'html2canvas';

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
const showAddServer = ref(false);
const host = ref('Remote Server');
const currentPath = ref('/');

// Context Menu State
const showContextMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);

// Tabs State: Only store metadata in Vue's reactive system.
const terminalTabs = ref<any[]>([]);
const activeTabId = ref<string | null>(null);
const activeTab = computed(() => terminalTabs.value.find(t => t.id === activeTabId.value));
const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

// SFTP / Data State
const realFiles = ref<any[]>([]);
const skills = ref<any[]>([]);
const webviewRef = ref<any>(null);

// Watch for tab switch: Auto-focus via Manager
watch(activeTabId, async (newId) => {
  if (newId) {
    await nextTick();
    setTimeout(() => {
      terminalManager.fit(newId);
      terminalManager.focus(newId);
    }, 50);
  }
});

// ==========================================
// --- TAB MANAGEMENT ---
// ==========================================
const createNewTab = (title = "Shell") => {
  const id = 'tab-' + Math.random().toString(36).substr(2, 9);
  
  // Register per-terminal callback for isolation
  terminalManager.setOnDataCallback(id, (data) => {
    if (isConnected.value) {
      invoke('write_pty', { data });
    }
  });

  terminalManager.getOrCreate(id);
  terminalTabs.value.push({ id, title, isBackground: false });
  activeTabId.value = id;
  nextTick(() => { setTimeout(() => terminalManager.focus(id), 50); });
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
  if (activeTab.value) {
    const selection = terminalManager.getSelection(activeTab.value.id).trim();
    activeTab.value.isBackground = true;
    // Use selection as semantic process name, fallback to task ID
    activeTab.value.title = selection 
      ? `Proc: ${selection.length > 20 ? selection.substring(0, 20) + '...' : selection}` 
      : `Task: ${activeTab.value.id.substr(0, 5)}`;
    activeTabId.value = null;
    createNewTab("New Shell");
  }
  showContextMenu.value = false;
};

const onTerminalContextMenu = (e: MouseEvent) => {
  menuX.value = e.clientX;
  menuY.value = e.clientY;
  showContextMenu.value = true;
};

// ==========================================
// --- CORE LOGIC: SSH Foundation ---
// ==========================================
const connectWithId = async (id: string) => { 
  if (isConnecting.value) return; 
  isConnecting.value = true;
  try {
    const s = savedServers.value.find(s => s.id === id);
    if (s) host.value = s.label || s.host;
    await invoke('connect_with_id', { id }); 
    await onConnected();
    backendLogs.value.push('[INFO] 已建立新会话。');
    // Immediate stats flow
    setTimeout(() => fetchStats(), 500);
  } catch (e) { alert("Connection Failed: " + e); } finally { isConnecting.value = false; }
};

const runSkill = async (skill: any) => {
  if (!isConnected.value) return;

  // Handle Context Requirement: Auto-screenshot for Manifest V2
  if (skill.context_requirement?.require_screenshot) {
    console.log("[SYSTEM] Skill requires UI context. Capturing...");
    await captureAndUpload(true);
  }

  const rpc = skill.rpc || skill.trigger;
  if (rpc) {
    if (rpc.includes('audit') || rpc.toLowerCase().includes('gemini') || rpc.includes('ter')) {
      isAutoPilot.value = true;
    }
    invoke('write_pty', { data: rpc.endsWith('\n') ? rpc : rpc + "\r\n" });
  }
};

const refreshExplorer = async () => {
  if (!isConnected.value) return;
  try {
    realFiles.value = await invoke('ls_remote', { path: currentPath.value });
  } catch (e) { console.error("SFTP refresh failed:", e); }
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

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  createNewTab("Main Shell");

  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<number[]>('pty-data', (event) => {
    const data = new Uint8Array(event.payload);
    const text = new TextDecoder().decode(data);
    if (isAutoPilot.value && text.includes('[TER_RPC]')) {
      try {
        const rpcMatch = text.match(/\[TER_RPC\]\s*({.*})/);
        if (rpcMatch && rpcMatch[1]) {
          const rpc = JSON.parse(rpcMatch[1]);
          if (rpc.action === 'screenshot') { captureAndUpload(true); return; }
          if (rpc.action === 'refresh_preview') { webviewRef.value?.reload(); return; }
        }
      } catch (e) {}
    }
    terminalManager.broadcast(data);
  });

  // Listen for dynamic port assignment from backend
  if (unlistenPort) unlistenPort();
  unlistenPort = await listen<number>('agent-tunnel-opened', (event) => {
    currentAgentPort.value = event.payload;
    backendLogs.value.push(`[SYSTEM] Agent tunnel established on port ${event.payload}`);
    // Trigger stats immediately once tunnel is open
    nextTick(() => fetchStats());
  });

  initCharts();
  setTimeout(() => {
    fetchStats();
    refreshExplorer();
    invoke('load_remote_skills').then((s: any) => skills.value = s).catch(e => console.error(e));
  }, 1000);
  setInterval(() => { fetchStats(); }, 3000);
};

// ==========================================
// --- CORE LOGIC: Visual Audit Loop ---
// ==========================================
const workspaceRef = ref<HTMLElement | null>(null);
const cyberPaneRef = ref<HTMLElement | null>(null);
const cyberWebviewRef = ref<HTMLElement | null>(null);

const captureAndUpload = async (auto = false) => {
  // 1. Smart Capture: Prioritize the webview viewport only
  // This ensures the AI sees only the relevant UI context (the web view)
  const target = (cyberWebviewRef.value && cyberWebviewRef.value.offsetParent !== null)
    ? cyberWebviewRef.value
    : (cyberPaneRef.value && cyberPaneRef.value.offsetParent !== null)
      ? cyberPaneRef.value
      : workspaceRef.value;
    
  if (!target) return;
  
  isAutoPilot.value = true;
  try {
    // 2. Capture Snapshot
    const canvas = await html2canvas(target, { 
      backgroundColor: '#000000', 
      useCORS: true, 
      scale: 2.0, // High resolution for AI vision
      logging: false,
      allowTaint: true,
      ignoreElements: (element) => element.classList.contains('terminal-pane') && target !== workspaceRef.value
    });
    
    const base64Data = canvas.toDataURL('image/png');
    const remotePath = await invoke<string>('upload_ui_snapshot', { base64Data });
    
    // 3. Dual-Sync Logs: Save last 10 JSON logs for transparency
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

// ==========================================
// --- UTILS ---
// ==========================================
const cpuChartRef = ref<HTMLElement | null>(null);
const memChartRef = ref<HTMLElement | null>(null);
let cpuChart: any, memChart: any;
const cpuHistory = ref<number[]>([]), memHistory = ref<number[]>([]);

const initCharts = () => { if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); if (memChartRef.value) memChart = echarts.init(memChartRef.value); };
const fetchStats = async () => {
  if (!currentAgentPort.value) return;
  try {
    const r = await fetch(`http://localhost:${currentAgentPort.value}/stats`, { 
      headers: { 'X-Ter-Token': agentToken.value },
      signal: AbortSignal.timeout(2000) // Safety timeout
    });
    if (!r.ok) throw new Error("Stats fetch failed");
    const d = await r.json();
    cpuHistory.value.push(d.cpu_usage); memHistory.value.push((d.mem_used / d.mem_total) * 100);
    if (cpuHistory.value.length > 30) { cpuHistory.value.shift(); memHistory.value.shift(); }
    cpuChart?.setOption(getChartOpt(cpuHistory.value, '#6366f1'));
    memChart?.setOption(getChartOpt(memHistory.value, '#a855f7'));
  } catch (e) {
    console.warn("Cyber metrics sync waiting for agent...");
  }
};
const getChartOpt = (d: any[], c: string) => ({ grid: { top: 5, bottom: 0, left: 0, right: 0 }, xAxis: { type: 'category', show: false }, yAxis: { type: 'value', min: 0, max: 100, show: false }, series: [{ data: d, type: 'line', smooth: true, areaStyle: { color: c }, itemStyle: { color: c }, showSymbol: false }], animation: false });

const masterPasswordStr = ref('');
const setMasterPass = async () => { await invoke('set_master_password', { password: masterPasswordStr.value }); isMasterPasswordSet.value = true; loadServers(); };
const loadServers = async () => { savedServers.value = await invoke('list_server_configs'); };

const newServer = ref({ label: '', host: '', user: '', pass: '', port: 22 });
const addServer = async () => {
  await invoke('save_server_config', { config: { id: Date.now().toString(), ...newServer.value, password_enc: newServer.value.pass, key_path: null } });
  showAddServer.value = false; loadServers();
};

let unlistenLog: any, unlistenPty: any, unlistenPort: any;
onMounted(async () => {
  unlistenLog = await listen<string>('backend-log', (e) => { backendLogs.value.push(e.payload); if (backendLogs.value.length > 100) backendLogs.value.shift(); });
  window.addEventListener('keydown', (e) => { if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; });
  window.addEventListener('focus', () => { if (activeTabId.value) terminalManager.focus(activeTabId.value); });
});
onUnmounted(() => { if (unlistenLog) unlistenLog(); if (unlistenPty) unlistenPty(); if (unlistenPort) unlistenPort(); });
</script>

<template>
  <div class="app-shell">
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" />

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
          <h3><span class="pulse"></span> Server Vault</h3>
          <button @click="showAddServer = true" class="btn-add">+</button>
        </header>
        <div class="server-list">
          <div v-for="s in savedServers" :key="s.id" class="server-card" @click="connectWithId(s.id)">
            <div class="icon-box">SSH</div>
            <div class="info"><b>{{ s.label }}</b><br/><small>{{ s.user }}@{{ s.host }}</small></div>
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

    <div v-else class="main-view">
      <SidebarPanel 
        :files="realFiles" 
        :bgTabs="backgroundTabs"
        :skills="skills"
        :cpuChartRef="(el: any) => cpuChartRef = el"
        :memChartRef="(el: any) => memChartRef = el"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="(id: string) => activeTabId = id"
        @switch-mode="(mode: number) => cyberMode = mode"
        @run-skill="runSkill"
        @change-dir="changeDir"
        @audit-ui="captureAndUpload(false)"
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
              {{ cyberMode === 1 ? 'Terminal Focus' : 'Cyber View' }}
            </button>
          </div>
        </nav>

        <div class="workspace-body">
          <section class="terminal-pane" :style="{ flex: cyberMode === 1 ? '0 0 50%' : '1' }">
            <TerminalTabs 
              :tabs="terminalTabs" 
              :activeTabId="activeTabId"
              @switch-tab="(id: string) => activeTabId = id"
              @close-tab="closeTab"
              @new-tab="createNewTab()"
              @terminal-context="onTerminalContextMenu"
            />
          </section>

          <section class="cyber-pane" v-if="cyberMode !== 0" :style="{ flex: '1' }" ref="cyberPaneRef">
            <div class="cyber-container">
              <!-- Logs View (Top 40%) -->
              <div class="cyber-logs-view">
                <header>
                  <span class="title">Cyber Transparency</span>
                  <span v-if="currentAgentPort" class="port-tag">PORT: {{ currentAgentPort }}</span>
                </header>
                <div class="logs-container" ref="logsScrollRef">
                  <div v-for="(log, i) in backendLogs" :key="i" class="log-line">
                    <span class="line-num">{{ i + 1 }}</span> {{ log }}
                  </div>
                  <div v-if="backendLogs.length === 0" class="empty-log">Waiting for agent JSON stream...</div>
                </div>
              </div>
              
              <div class="cyber-divider"></div>

              <!-- Webview (Bottom 60%) -->
              <div class="cyber-webview-wrapper" ref="cyberWebviewRef">
                <CyberWebview 
                  ref="webviewRef" 
                  :url="`http://localhost:${currentAgentPort || 5173}`" 
                />
              </div>
            </div>
          </section>
        </div>
      </main>
    </div>
  </div>
</template>

<style scoped>
.cyber-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #000;
}

.cyber-logs-view {
  flex: 0 0 40%;
  display: flex;
  flex-direction: column;
  background: #0a0a0a;
  border-bottom: 1px solid #1a1a1c;
  overflow: hidden;
}

.cyber-logs-view header {
  padding: 8px 12px;
  background: #0c0c0e;
  border-bottom: 1px solid #1a1a1c;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.cyber-logs-view .title {
  font-size: 10px;
  color: #6366f1;
  font-weight: bold;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.port-tag {
  font-size: 9px;
  color: #22c55e;
  background: rgba(34, 197, 94, 0.1);
  padding: 1px 6px;
  border-radius: 4px;
  font-family: 'JetBrains Mono', monospace;
}

.logs-container {
  flex: 1;
  padding: 10px;
  overflow-y: auto;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: #a1a1aa;
}

.log-line {
  margin-bottom: 2px;
  white-space: pre-wrap;
  word-break: break-all;
}

.line-num { color: #3f3f46; margin-right: 8px; user-select: none; }

.empty-log {
  color: #3f3f46;
  text-align: center;
  margin-top: 20px;
  font-style: italic;
}

.cyber-divider {
  height: 1px;
  background: #1a1a1c;
  box-shadow: 0 0 10px rgba(0,0,0,0.5);
}

.cyber-webview-wrapper {
  flex: 1;
  background: #000;
  position: relative;
}

.context-menu { position: fixed; z-index: 10000; background: #18181b; border: 1px solid #3f3f46; border-radius: 6px; padding: 4px; min-width: 150px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; border-radius: 4px; transition: 0.2s; }
.menu-item:hover { background: #3f3f46; color: #fff; }
.menu-item.disabled { color: #52525b; cursor: not-allowed; }
.menu-divider { height: 1px; background: #27272a; margin: 4px 0; }
.app-shell { height: 100vh; background: #050505; color: #e4e4e7; font-family: 'Inter', system-ui; overflow: hidden; position: relative; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; position: relative; }
.tool-bar { height: 45px; background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }
.workspace-body { flex: 1; display: flex; overflow: hidden; }
.terminal-pane { height: 100%; display: flex; flex-direction: column; transition: flex 0.3s ease; position: relative; min-width: 0; min-height: 0; }
.cyber-pane { height: 100%; border-left: 1px solid #1a1a1c; overflow: hidden; background: #000; }
.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; background: radial-gradient(circle at center, #111 0%, #000 100%); }
.vault-container { width: 450px; background: #111; border: 1px solid #333; border-radius: 12px; padding: 25px; box-shadow: 0 20px 50px rgba(0,0,0,0.8); position: relative; overflow: hidden; }
.server-card { background: #1a1a1a; border: 1px solid #333; padding: 12px; border-radius: 8px; display: flex; align-items: center; cursor: pointer; transition: 0.2s; margin-bottom: 10px; }
.pulse { display: inline-block; width: 8px; height: 8px; background: #d946ef; border-radius: 50%; margin-right: 8px; box-shadow: 0 0 10px #d946ef; animation: pulse-anim 2s infinite; }
@keyframes pulse-anim { 0% { opacity: 0.4; transform: scale(0.8); } 50% { opacity: 1; transform: scale(1.1); } 100% { opacity: 0.4; transform: scale(0.8); } }
.connecting-mask { position: absolute; inset: 0; background: rgba(0,0,0,0.8); backdrop-filter: blur(4px); display: flex; flex-direction: column; align-items: center; justify-content: center; z-index: 10; }
.spinner { width: 30px; height: 30px; border: 3px solid #333; border-top-color: #6366f1; border-radius: 50%; animation: spin 1s linear infinite; margin-bottom: 15px; }
@keyframes spin { to { transform: rotate(360deg); } }
.glass { backdrop-filter: blur(10px); background: rgba(20,20,25,0.8); }
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 10000; }
.auth-card { background: #111; padding: 30px; border-radius: 12px; border: 1px solid #333; width: 320px; }
.auth-card input { width: 100%; padding: 12px; background: #000; border: 1px solid #333; color: #fff; border-radius: 6px; margin-bottom: 15px; }
.btn-primary { width: 100%; padding: 12px; background: #6366f1; border: none; color: #fff; border-radius: 6px; cursor: pointer; font-weight: bold; }
.modal-btns { display: flex; gap: 10px; }
.btn-ghost { flex: 1; padding: 10px; background: transparent; border: 1px solid #333; color: #71717a; border-radius: 6px; cursor: pointer; }
.btn-tool { background: transparent; border: 1px solid #27272a; color: #a1a1aa; padding: 4px 12px; border-radius: 4px; cursor: pointer; font-size: 11px; margin-left: 10px; }
.btn-tool:hover { border-color: #6366f1; color: #fff; }
</style>
