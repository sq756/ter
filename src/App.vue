<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Components
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import CyberWebview from './components/CyberWebview.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import CyberGate from './components/CyberGate.vue';

// Composables
import { useMorse } from './composables/useMorse';
import { useTabs } from './composables/useTabs';
import { useStats } from './composables/useStats';

// ==========================================
// --- GLOBAL STATE ---
// ==========================================
const isConnected = ref(false);
const host = ref('Remote Server');
const isAutoPilot = ref(false);
const lastAutoPilotTime = ref(0);
const connectionStatus = ref<'connected' | 'busy' | 'disconnected'>('disconnected');
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const showSettings = ref(false);
const activeMacros = ref<{name: string, cmd: string}[]>([]);

const isLocked = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);
const previewUrl = ref('http://localhost:5173');
const isWebviewLoading = ref(false);
const backendLogs = ref<string[]>([]);
const currentPath = ref('/');
const realFiles = ref<any[]>([]);
const skills = ref<any[]>([]);

const showContextMenu = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const contextMenuTabId = ref<string | null>(null);
const hasErrorSelection = ref(false);

const storageKey = computed(() => `ter_tabs_${host.value.replace(/\s+/g, '_')}`);
let statsIntervalId: any = null;

// ==========================================
// --- DECOUPLED LOGIC ---
// ==========================================
const { 
  terminalTabs, activeTabId, backgroundTabs, 
  createNewTab, closeTab, sendToBackground, bringToForeground, renameTab 
} = useTabs(isConnected, backendLogs);

const { 
  cpuChartRef, memChartRef, currentCpuUsage, initCharts, fetchStats 
} = useStats(currentAgentPort, agentToken);

const calculateMenuPosition = (e: MouseEvent, estimatedHeight = 250, estimatedWidth = 160) => {
  let x = e.clientX, y = e.clientY;
  if (y + estimatedHeight > window.innerHeight) y = window.innerHeight - estimatedHeight - 10;
  if (x + estimatedWidth > window.innerWidth) x = window.innerWidth - estimatedWidth - 10;
  menuX.value = x; menuY.value = y;
};

const { 
  morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters,
  handleMorseMouse, handleMorseWheel, onMorseMacro
} = useMorse(activeTabId, calculateMenuPosition);

// ==========================================
// --- WATCHERS ---
// ==========================================
watch(terminalTabs, (val) => { if (isConnected.value) localStorage.setItem(storageKey.value, JSON.stringify(val)); }, { deep: true });
watch(activeTriggers, (val) => { localStorage.setItem('ter_active_triggers', JSON.stringify(val)); }, { deep: true });

// ==========================================
// --- METHODS ---
// ==========================================
const viewHistory = async (originalTabId: string) => {
  const t = terminalTabs.value.find(x => x.id === originalTabId);
  const playbackId = await createNewTab(`Playback: ${t?.title || originalTabId}`, true);
  try {
    const logs = await invoke<number[][]>('get_terminal_logs', { tabId: originalTabId, limit: 1000 });
    for (const chunk of logs) { 
      terminalManager.write(playbackId, new Uint8Array(chunk)); 
      await new Promise(r => setTimeout(r, 20)); 
    }
  } catch (e) { terminalManager.write(playbackId, `\r\n[ERROR] History Fail: ${e}\r\n`); }
};

const copySelectedText = async () => { 
  const id = contextMenuTabId.value || activeTabId.value; 
  if (id) { 
    const s = terminalManager.getSelection(id); 
    if (s) await navigator.clipboard.writeText(s); 
  } 
  showContextMenu.value = false; 
};

const pasteFromClipboard = async () => { 
  const id = contextMenuTabId.value || activeTabId.value; 
  if (id) { 
    try { 
      const t = await navigator.clipboard.readText(); 
      if (t) invoke('write_pty', { tabId: id, data: t }); 
    } catch(e){} 
  } 
  showContextMenu.value = false; 
};

const onTerminalContextMenu = (p: { e: MouseEvent, id: string }) => { 
  contextMenuTabId.value = p.id; 
  calculateMenuPosition(p.e); 
  const s = terminalManager.getSelection(p.id); 
  hasErrorSelection.value = s.toLowerCase().includes('error') || s.toLowerCase().includes('exception') || s.includes('\x1b[31m'); 
  showContextMenu.value = true; 
};

const captureAndUpload = async (auto = false) => {
  backendLogs.value.push(`[SYSTEM] Initiating UI sync...`);
  try {
    const path = await invoke<string>('ai_audit_ui');
    if (!auto) backendLogs.value.push(`[INFO] UI Snapshot saved: ${path}`);
  } catch (e) {
    backendLogs.value.push(`[ERROR] Audit Fail: ${e}`);
  }
};

const onConnected = async (hostLabel: string) => {
  host.value = hostLabel;
  isConnected.value = true;
  connectionStatus.value = 'connected';
  try { agentToken.value = await invoke('get_agent_token'); } catch(e){}
  
  const saved = localStorage.getItem(storageKey.value);
  if (saved) {
    try {
      const ts = JSON.parse(saved); 
      terminalTabs.value = ts;
      for (const t of ts) {
        await createNewTab(t.title, false, t.id);
      }
      activeTabId.value = ts.find((t: any) => !t.isBackground)?.id || ts[0]?.id;
    } catch (e) { await createNewTab("Main Shell", false, "tab-1"); }
  } else if (terminalTabs.value.length === 0) { await createNewTab("Main Shell", false, "tab-1"); }

  setTimeout(() => {
    refreshExplorer();
    invoke('load_remote_skills').then((s: any) => skills.value = s).catch(()=>{});
    nextTick(() => {
      initCharts();
      if (statsIntervalId) clearInterval(statsIntervalId);
      statsIntervalId = setInterval(fetchStats, 3000);
    });
  }, 1000);
};

const refreshExplorer = async () => { if (isConnected.value) realFiles.value = await invoke('ls_remote', { path: currentPath.value }); };
const changeDir = (p: string) => {
  if (p === '..') { const pts = currentPath.value.split('/').filter(x => x); pts.pop(); currentPath.value = '/' + pts.join('/'); } 
  else { currentPath.value = (currentPath.value === '/' ? '' : currentPath.value) + '/' + p; }
  const s = localStorage.getItem('ter_fast_access'); let l = s ? JSON.parse(s) : []; 
  l = [currentPath.value, ...l.filter((x: string) => x !== currentPath.value)].slice(0, 5); 
  localStorage.setItem('ter_fast_access', JSON.stringify(l)); 
  refreshExplorer();
};

const onFastAccess = async (p: string) => { 
  currentPath.value = p; 
  if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${p}"\r` }); 
  refreshExplorer(); 
};

const refreshWebview = async (fUrl?: string) => {
  if (fUrl) previewUrl.value = fUrl; let u = previewUrl.value.trim(); if (!u) return; if (/^\d+$/.test(u)) { u = `http://localhost:${u}`; previewUrl.value = u; }
  const m = u.match(/(?:localhost|127\.0\.0\.1):(\d+)/); 
  if (m && m[1]) { 
    isWebviewLoading.value = true; 
    try { 
      const p = await invoke<number>('open_dynamic_tunnel', { remotePort: parseInt(m[1]) }); 
      previewUrl.value = `http://localhost:${p}`; 
    } catch (e) {} finally { isWebviewLoading.value = false; } 
  }
};

const handleExtractDOM = async () => { backendLogs.value.push(`[INFO] Extracting DOM...`); await invoke('extract_cyber_dom'); };
const onDomExtracted = async (md: string) => { if (activeTabId.value) { await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~${md}\x1b[201~\r` }); backendLogs.value.push(`[INFO] Snapshot injected.`); } };

const runMacro = async (c: string) => { if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: c + '\n' }); showMorseMacro.value = false; };
const renameTabAction = () => { 
  const id = contextMenuTabId.value; 
  if (id) { 
    const n = prompt("New name:"); 
    if (n) renameTab(id, n); 
  } 
  showContextMenu.value = false; 
};

const copyTabIdAction = async () => { if (contextMenuTabId.value) await navigator.clipboard.writeText(contextMenuTabId.value); showContextMenu.value = false; };
const diagnoseSelection = async () => { 
  const id = contextMenuTabId.value || activeTabId.value; 
  if (id) { 
    const s = terminalManager.getSelection(id); 
    if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~帮我诊断并给方案：\n\n\`\`\`\n${s}\n\`\`\`\x1b[201~\r` }); 
  } 
  showContextMenu.value = false; 
};

const runSkill = async (skill: any) => {
  if (!isConnected.value) return;
  if (skill.context_requirement?.require_screenshot) {
    await captureAndUpload(true);
  }
  const rpc = skill.rpc || skill.trigger;
  if (rpc && activeTabId.value) {
    invoke('write_pty', { tabId: activeTabId.value, data: rpc.endsWith('\n') ? rpc : rpc + "\r\n" });
  }
};

let unlistenLog: any, unlistenPty: any;
const preventDefaultContextMenu = (e: MouseEvent) => e.preventDefault();
const handleGlobalKeyDown = (e: KeyboardEvent) => { if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; };

onMounted(async () => {
  window.addEventListener('contextmenu', preventDefaultContextMenu);
  window.addEventListener('keydown', handleGlobalKeyDown);
  
  const st = localStorage.getItem('ter_active_triggers'); if (st) try { activeTriggers.value = JSON.parse(st); } catch(e){}
  const sm = localStorage.getItem('ter_macros'); if (sm) try { activeMacros.value = JSON.parse(sm); } catch(e){}
  
  unlistenLog = await listen<string>('backend-log', (e) => { 
    backendLogs.value.push(e.payload); 
    if (backendLogs.value.length > 500) backendLogs.value.shift(); 
  });
  
  const decoder = new TextDecoder('utf-8', { fatal: false });
  unlistenPty = await listen<any>('pty-data', (ev) => {
    const { id, data } = ev.payload;
    let bytes = typeof data === 'string' ? new TextEncoder().encode(data) : new Uint8Array(data);
    let text = decoder.decode(bytes);

    // ==========================================
    // --- PTY RPC INTERCEPTOR (Enhanced) ---
    // ==========================================
    if (text.includes('[TER_RPC]')) {
      const rpcRegex = /\[TER_RPC\]\s*({.*?})/g;
      let match;
      let cleanedText = text;
      let foundRpc = false;

      while ((match = rpcRegex.exec(text)) !== null) {
        if (!match[1]) continue;
        try {
          const rpc = JSON.parse(match[1]);
          foundRpc = true;
          
          if (rpc.action === 'screenshot') {
            captureAndUpload(true);
          } else if (rpc.action === 'notify') {
            backendLogs.value.push(`[🔔 AI NOTIFY] ${rpc.msg || rpc.message}`);
          } else if (rpc.action === 'chart') {
            backendLogs.value.push(`[📊 AI CHART DATA] ${JSON.stringify(rpc.data)}`);
          }
          
          // Remove the RPC command from the text stream
          cleanedText = cleanedText.replace(match[0], '');
        } catch (e) { console.warn("RPC Parse Error:", e); }
      }

      if (foundRpc) {
        if (cleanedText.trim() === '') return; // Stop if nothing left
        bytes = new TextEncoder().encode(cleanedText);
      }
    }

    if (terminalManager) terminalManager.write(id, bytes);
    
    if (connectionStatus.value === 'connected') { 
      connectionStatus.value = 'busy'; 
      setTimeout(() => { if (isConnected.value) connectionStatus.value = 'connected'; }, 200); 
    }
    
    if (isAutoPilot.value && id === activeTabId.value) {
      const pt = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
      const actionMatch = pt.match(/\[TER_ACTION:\s*(click|type)\((\d+)(?:,\s*"(.*?)")?\)\]/);
      if (actionMatch) {
        const action = actionMatch[1], eid = actionMatch[2], txt = actionMatch[3] || "";
        const code = action === 'click' ? `window.TerAgent.click(${eid})` : `window.TerAgent.type(${eid}, ${JSON.stringify(txt)})`;
        invoke('eval_cyber_webview', { code });
      } else if (!pt.includes('tab-') && (Date.now() - lastAutoPilotTime.value) > 500) {
        const lm = pt.match(/http:\/\/localhost:(\d+)/); 
        if (lm && lm[1]) refreshWebview(`http://localhost:${lm[1]}`);
        if (activeTriggers.value.some(t => pt.includes(t))) { 
          lastAutoPilotTime.value = Date.now(); 
          setTimeout(() => { invoke('write_pty', { tabId: id, data: "\r" }); }, 300); 
        }
      }
    }
  });
});

onUnmounted(() => {
  window.removeEventListener('contextmenu', preventDefaultContextMenu);
  window.removeEventListener('keydown', handleGlobalKeyDown);
  if (unlistenLog) unlistenLog(); if (unlistenPty) unlistenPty();
  if (statsIntervalId) clearInterval(statsIntervalId);
});
</script>

<template>
  <div class="app-shell" @click="showContextMenu = false; showMorseMacro = false">
    <CyberGate v-if="!isConnected" @connected="onConnected" />
    
    <div v-else class="main-view">
      <SettingsPanel :isOpen="showSettings" @close="showSettings = false" @update-macros="(m) => activeMacros = m" />
      <SidebarPanel 
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :cpuChartRef="cpuChartRef" :memChartRef="memChartRef"
        v-model:isAutoPilot="isAutoPilot"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
      />

      <main class="workspace" ref="workspaceRef" @click="activeTabId && terminalManager.focus(activeTabId)">
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename Tab</div><div class="menu-item" @click="copyTabIdAction">🆔 Copy ID</div><div class="menu-item" @click="sendToBackground(contextMenuTabId)">🚀 Background</div>
          <div class="menu-divider"></div><div class="menu-item" @click="copySelectedText">📋 Copy</div><div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div><div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Force Close</div>
        </div>
        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
          <div class="menu-divider"></div><div class="menu-item" @click="showSettings = true">⚙️ Manage...</div>
        </div>
        <div v-if="morseSequence || morseText" class="morse-preview-overlay">
          <div class="sequence">{{ morseSequence }}</div>
          <div class="text">{{ morseText }}</div>
          <div class="candidates" v-if="possibleLetters">{{ possibleLetters }}</div>
        </div>
        <nav class="tool-bar"><div class="status-chip"><span class="pulse purple"></span> {{ host }}</div><div class="actions"><button @click="isLocked = true" class="btn-tool">Lock System</button></div></nav>
        <div class="workspace-body">
          <section class="terminal-pane"><TerminalTabs :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" @terminal-context="onTerminalContextMenu" /></section>
          <section class="cyber-pane" v-if="cyberMode !== 0">
            <div class="cyber-container">
              <div class="cyber-logs-view"><header><span class="title">Cyber Logs</span></header><div class="logs-container"><div v-for="(log, i) in backendLogs" :key="i" class="log-line"><span class="line-num">{{ i + 1 }}</span> {{ log }}</div></div></div>
              <div class="cyber-divider"></div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <div class="address-input-wrapper"><span class="secure-icon">🔒</span><input v-model="previewUrl" @keyup.enter="refreshWebview()" @focus="($event.target as HTMLInputElement).select()" class="address-bar-input" /></div>
                  <button class="refresh-btn" @click="refreshWebview()" :class="{ spinning: isWebviewLoading }">{{ isWebviewLoading ? '⏳' : '⚡' }}</button>
                  <button class="extract-btn" @click="handleExtractDOM">👁️ Extract</button>
                </nav>
                <CyberWebview :url="previewUrl" @dom-extracted="onDomExtracted" />
              </div>
            </div>
          </section>
        </div>
        <footer class="status-bar">
          <div class="status-left stealth-zone" @mousedown.prevent="handleMorseMouse" @wheel.prevent="handleMorseWheel" @contextmenu.prevent="onMorseMacro">
            <div class="tiny-dot" :class="{ 'active': isMorsePressed }"></div>
            <span class="item">1 | Agent: Active</span>
          </div>
          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload(false)">📸 Audit</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">{{ cyberMode === 1 ? '🖥️' : '🌐' }}</button>
            <div class="status-toggle"><span>Auto</span><label class="mini-switch"><input type="checkbox" v-model="isAutoPilot" /><span class="slider"></span></label></div>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
  </div>
</template>

<style scoped>
.app-shell { height: 100vh; background: #000; color: #d4d4d8; font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace, 'Segoe UI Emoji', 'Noto Color Emoji'; overflow: hidden; }
.main-view { display: flex; height: 100%; width: 100%; }
.workspace { flex: 1; display: flex; flex-direction: column; background: #000; overflow: hidden; min-width: 0; }

.icon, .file-icon, .btn-tool, .status-btn { 
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji';
}

.context-menu { position: fixed; z-index: 1000000; background: #09090b; border: 1px solid #22c55e; padding: 4px; min-width: 160px; box-shadow: 0 10px 25px rgba(0,0,0,0.8); }
.menu-header { padding: 6px 12px; font-size: 9px; color: #166534; border-bottom: 1px solid #18181b; margin-bottom: 4px; }
.menu-item { padding: 8px 12px; font-size: 11px; color: #d4d4d8; cursor: pointer; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-item.danger { color: #ef4444; }
.menu-item.danger:hover { background: #ef4444; color: #000; }
.menu-divider { height: 1px; background: #18181b; margin: 4px 0; }

.status-bar { height: 28px; background: #000; border-top: 1px solid #18181b; color: #52525b; display: flex; justify-content: space-between; align-items: center; padding: 0 12px; font-size: 10px; z-index: 100; flex-shrink: 0; }
.status-right { display: flex; align-items: center; }
.status-right > * { margin-left: 15px !important; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; transition: all 0.1s; }
.tiny-dot.active { transform: scale(1.1); box-shadow: 0 0 8px #22c55e; filter: brightness(1.5); }
.stealth-zone { display: flex; align-items: center; gap: 8px; cursor: pointer; height: 100%; padding: 0 5px; }
.stealth-zone:hover { background: rgba(34, 197, 94, 0.05); }

.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; position: relative; display: flex; flex-direction: column; overflow: hidden; }
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; background: #000; overflow: hidden; display: flex; flex-direction: column; }
.cyber-container { display: flex; flex-direction: column; height: 100%; flex: 1; overflow: hidden; }
.cyber-logs-view { flex: 0 0 30%; display: flex; flex-direction: column; background: #000; border-bottom: 1px solid #27272a; overflow: hidden; }
.logs-container { flex: 1; padding: 10px; overflow-y: auto; font-family: 'JetBrains Mono', monospace; font-size: 10px; color: #a1a1aa; }
.cyber-webview-wrapper { flex: 1; background: #000; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { height: 32px; border-bottom: 1px solid #27272a; display: flex; align-items: center; padding: 0 8px; gap: 8px; }
.address-input-wrapper { flex: 1; background: #050505; border: 1px solid #18181b; height: 24px; animation: breathing-border 3s infinite; display: flex; align-items: center; padding: 0 8px; }
@keyframes breathing-border { 0% { border-color: #18181b; } 50% { border-color: #22c55e; } 100% { border-color: #18181b; } }
.address-bar-input { background: transparent; border: none; color: #22c55e; font-size: 10px; width: 100%; outline: none; font-family: 'JetBrains Mono', monospace; }
.extract-btn { background: rgba(168, 85, 247, 0.1); border: 1px solid #a855f7; color: #a855f7; font-size: 10px; padding: 2px 8px; cursor: pointer; font-family: 'JetBrains Mono', monospace; }
.extract-btn:hover { background: rgba(168, 85, 247, 0.2); box-shadow: 0 0 10px #a855f7; }

.morse-preview-overlay { position: absolute; bottom: 40px; left: 10px; background: rgba(0, 0, 0, 0.9); border: 1px solid #22c55e; padding: 10px 20px; z-index: 1000; display: flex; flex-direction: column; align-items: center; pointer-events: none; }
.morse-preview-overlay .sequence { font-size: 24px; color: #22c55e; letter-spacing: 4px; }
.morse-preview-overlay .candidates { font-size: 9px; color: #166534; margin-top: 5px; }

.tool-bar { height: 36px; background: #000; border-bottom: 1px solid #18181b; display: flex; align-items: center; justify-content: space-between; padding: 0 15px; }
.btn-tool { background: transparent; border: 1px solid #27272a; color: #52525b; padding: 3px 10px; font-size: 10px; cursor: pointer; text-transform: uppercase; }
.btn-tool:hover { border-color: #ef4444; color: #ef4444; }
.status-chip { font-size: 11px; color: #52525b; display: flex; align-items: center; gap: 8px; }
.pulse.purple { width: 6px; height: 6px; background: #a855f7; border-radius: 50%; box-shadow: 0 0 5px #a855f7; }

.mini-switch { position: relative; display: inline-block; width: 24px; height: 12px; }
.mini-switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #27272a; transition: .4s; border-radius: 12px; }
.slider:before { position: absolute; content: ""; height: 8px; width: 8px; left: 2px; bottom: 2px; background-color: white; transition: .4s; border-radius: 50%; }
input:checked + .slider { background-color: #3b82f6; }
input:checked + .slider:before { transform: translateX(12px); }
.status-toggle { display: flex; align-items: center; gap: 8px; font-size: 10px; color: #52525b; }
.status-btn { background: transparent; border: none; color: #52525b; cursor: pointer; font-size: 10px; padding: 2px 6px; border-radius: 4px; transition: all 0.2s; }
.status-btn:hover { color: #fff; }
</style>
