<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as echarts from 'echarts';
import html2canvas from 'html2canvas';

// Import New Sub-components
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';

// ==========================================
// --- GLOBAL STATE: The Heart of Ter ---
// ==========================================
const isConnected = ref(false);
const isConnecting = ref(false); 
const isMasterPasswordSet = ref(false);
const isAutoPilot = ref(false); 
const isLocked = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const backendLogs = ref<string[]>([]);
const savedServers = ref<any[]>([]);
const showAddServer = ref(false);
const host = ref('Remote Server');

// Tabs State
const terminalTabs = ref<any[]>([]);
const activeTabId = ref<string | null>(null);
const activeTab = computed(() => terminalTabs.value.find(t => t.id === activeTabId.value));
const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

// SFTP / Data State
const realFiles = ref<any[]>([]);

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
  } catch (e) { alert("Connection Failed: " + e); } finally { isConnecting.value = false; }
};

const onConnected = async () => {
  isConnected.value = true;
  agentToken.value = await invoke('get_agent_token');
  
  // Create first tab
  createNewTab("Main Shell");

  if (unlistenPty) unlistenPty();
  unlistenPty = await listen<number[]>('pty-data', (event) => {
    const data = new Uint8Array(event.payload);
    const text = new TextDecoder().decode(data);

    // --- RPC INTERCEPTOR (The Central Nervous System) ---
    if (isAutoPilot.value && text.includes('[TER_RPC]')) {
      try {
        const rpcMatch = text.match(/\[TER_RPC\]\s*({.*})/);
        if (rpcMatch && rpcMatch[1]) {
          const rpc = JSON.parse(rpcMatch[1]);
          if (rpc.action === 'screenshot') {
            captureAndUpload(true);
            return; // Block signal from terminal
          }
        }
      } catch (e) {}
    }
    // ----------------------------------------------------

    if (activeTab.value?.instance) activeTab.value.instance.write(data);
  });

  initCharts();
  setInterval(() => { fetchStats(); }, 3000);
};

// ==========================================
// --- CORE LOGIC: Visual Audit Loop ---
// ==========================================
const workspaceRef = ref<HTMLElement | null>(null);
const captureAndUpload = async (auto = false) => {
  if (!workspaceRef.value) return;
  const canvas = await html2canvas(workspaceRef.value, { backgroundColor: '#000' });
  const base64 = canvas.toDataURL('image/png');
  const remotePath = await invoke<string>('upload_ui_snapshot', { base64Data: base64 });
  const msg = auto ? `[SYSTEM] Auto-Snap: ${remotePath}` : `Review ${remotePath}`;
  await invoke('write_pty', { data: msg + "\r" });
};

// ==========================================
// --- TAB MANAGEMENT ---
// ==========================================
const createNewTab = (title = "Shell") => {
  const id = 'tab-' + Math.random().toString(36).substr(2, 9);
  const term: any = null; // Actual instance created in factory or subcomponent
  terminalTabs.value.push({ id, title, instance: term, isBackground: false });
  activeTabId.value = id;
};

const sendToBackground = () => {
  if (activeTab.value) {
    activeTab.value.isBackground = true;
    activeTab.value.title = "Task: " + activeTab.value.id.substr(0,5);
    activeTabId.value = null;
    createNewTab("New Shell");
  }
};

// ==========================================
// --- UTILS: Charts & Lifecycle ---
// ==========================================
const cpuChartRef = ref<HTMLElement | null>(null);
const memChartRef = ref<HTMLElement | null>(null);
let cpuChart: any, memChart: any;
const cpuHistory = ref<number[]>([]), memHistory = ref<number[]>([]);

const initCharts = () => { if (cpuChartRef.value) cpuChart = echarts.init(cpuChartRef.value); if (memChartRef.value) memChart = echarts.init(memChartRef.value); };
const fetchStats = async () => {
  try {
    const r = await fetch(`http://localhost:54321/stats`, { headers: { 'X-Ter-Token': agentToken.value } });
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

const newServer = ref({ label: '', host: '', user: '', pass: '', port: 22 });
const addServer = async () => {
  await invoke('save_server_config', { config: { id: Date.now().toString(), ...newServer.value, password_enc: newServer.value.pass, key_path: null } });
  showAddServer.value = false; loadServers();
};

let unlistenLog: any, unlistenPty: any;
onMounted(async () => {
  unlistenLog = await listen<string>('backend-log', (e) => { backendLogs.value.push(e.payload); if (backendLogs.value.length > 100) backendLogs.value.shift(); });
  window.addEventListener('keydown', (e) => { if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; });
});
onUnmounted(() => { if (unlistenLog) unlistenLog(); if (unlistenPty) unlistenPty(); });
</script>

<template>
  <div class="app-shell">
    
    <!-- Visual Sensation Layer -->
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" />

    <!-- Phase 1: Unlock -->
    <div v-if="!isMasterPasswordSet" class="modal-overlay">
      <div class="auth-card">
        <h2>🔒 Unlock Vault</h2>
        <input v-model="masterPasswordStr" type="password" placeholder="Password..." @keyup.enter="setMasterPass" />
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
            <div class="info"><b>{{ s.label }}</b><br/><small>{{ s.user }}@{{ s.host }}</small></div>
          </div>
        </div>
        <div v-if="isConnecting" class="connecting-mask"><div class="spinner"></div><p>Tunneling...</p></div>
      </div>
      <!-- Add Server Modal -->
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
      
      <!-- Componentized Sidebar -->
      <SidebarPanel 
        :files="realFiles" 
        :bgTabs="backgroundTabs"
        :cpuChartRef="(el: any) => cpuChartRef = el"
        :memChartRef="(el: any) => memChartRef = el"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="(id: string) => activeTabId = id"
        @audit-ui="captureAndUpload(false)"
      />

      <main class="workspace" ref="workspaceRef">
        <nav class="tool-bar">
          <div class="status-chip"><span class="pulse purple"></span> {{ host }}</div>
          <div class="actions">
            <button @click="isLocked = true" class="btn-tool">Lock</button>
            <button @click="cyberMode = (cyberMode + 1) % 4" class="btn-tool">Cyber</button>
          </div>
        </nav>

        <!-- Componentized Terminal Workspace -->
        <TerminalTabs 
          :tabs="terminalTabs" 
          :activeTabId="activeTabId"
          @switch-tab="(id: string) => activeTabId = id"
          @new-tab="createNewTab()"
          @terminal-context="sendToBackground()"
        />
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-shell { height: 100vh; background: #050505; color: #e4e4e7; font-family: 'Inter', system-ui; overflow: hidden; position: relative; }

/* Main UI Layout */
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; }
.tool-bar { height: 45px; background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }

/* Preserved Core Styles */
.workspace-setup { height: 100%; display: flex; align-items: center; justify-content: center; background: radial-gradient(circle at center, #111 0%, #000 100%); }
.vault-container { width: 450px; background: #111; border: 1px solid #333; border-radius: 12px; padding: 25px; box-shadow: 0 20px 50px rgba(0,0,0,0.8); position: relative; overflow: hidden; }
.server-card { background: #1a1a1a; border: 1px solid #333; padding: 12px; border-radius: 8px; display: flex; align-items: center; cursor: pointer; transition: 0.2s; margin-bottom: 10px; }
.server-card:hover { border-color: #6366f1; }
.icon-box { background: #333; color: #6366f1; font-size: 10px; font-weight: bold; padding: 4px 8px; border-radius: 4px; margin-right: 15px; }
.btn-add { background: #6366f1; border: none; color: white; width: 30px; height: 30px; border-radius: 6px; cursor: pointer; }
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
</style>
