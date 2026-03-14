<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch, computed, shallowRef } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

const appWindow = getCurrentWindow();

// Components
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import CyberGate from './components/CyberGate.vue';
import NetworkMatrix from './components/NetworkMatrix.vue';
import CyberWebview from './components/CyberWebview.vue';

// Composables
import { useMorse } from './composables/useMorse';
import { useTabs } from './composables/useTabs';
import { useStats } from './composables/useStats';
import { useExplorer, sanitizeSftpPath } from './composables/useExplorer';
import { useExplorerContextMenu } from './composables/useExplorerContextMenu';
import { useCyber } from './composables/useCyber';
import { useContextMenu } from './composables/useContextMenu';
import { usePtyListener } from './composables/usePtyListener';
import { useWebviews } from './composables/useWebviews';
import { useBookmarks } from './composables/useBookmarks';
import { useUIPreferences } from './composables/useUIPreferences';

// ==========================================
// --- GLOBAL STATE ---
// ==========================================
const isConnected = ref(false);
const host = ref('Remote Server');
const activeServerId = ref<string | null>(null);
const hostId = computed(() => isConnected.value ? host.value : 'GLOBAL');
const isAutoPilot = ref(false);
const lastAutoPilotTime = ref(0);
const connectionStatus = ref<'connected' | 'busy' | 'disconnected'>('disconnected');
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const showSettings = ref(false);
const activeMacros = ref<{name: string, cmd: string}[]>([]);

const isLocked = ref(false);
const isSafeMode = ref(localStorage.getItem('ter_safe_mode') === 'true');
const showNetworkMatrix = ref(false);
const isSidebarOpen = ref(true);
const isCtrlPressed = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);

// v2.11.44: Log Throttling & shallowRef to prevent UI freezing
const backendLogs = shallowRef<string[]>([]);
const logQueue: string[] = [];
let logThrottleId: any = null;

const processLogQueue = () => {
  if (logQueue.length > 0) {
    const newLogs = [...backendLogs.value, ...logQueue];
    backendLogs.value = newLogs.length > 500 ? newLogs.slice(-500) : newLogs;
    logQueue.length = 0;
  }
  logThrottleId = null;
};
const isLogsPaused = ref(false);
const skills = ref<any[]>([]);

// v2.11.46: Tactical HUD Optimization
const tacticalLogs = computed(() => backendLogs.value.slice(-50));
const isTrafficFlashing = ref(false);
let trafficTimeout: any = null;

const getLogColor = (log: string) => {
  if (log.includes('[ERROR]')) return '#ef4444';
  if (log.includes('[SYSTEM]') || log.includes('[STATUS]')) return '#22c55e';
  if (log.includes('[DEBUG]') || log.includes('[INFO]')) return '#52525b'; // Dim Gray for background noise
  if (log.includes('AI') || log.includes('Reasoning')) return '#a855f7';
  return '#a1a1aa';
};

// ...

// v2.10.3: Resizable SFTP
const sftpHeight = ref(200);
const isResizingSFTP = ref(false);

const startResizingSFTP = (e: MouseEvent) => {
  isResizingSFTP.value = true;
  document.body.style.cursor = 'ns-resize';
};

const stopResizingSFTP = () => {
  isResizingSFTP.value = false;
  document.body.style.cursor = '';
};

const handleGlobalMouseMove = (e: MouseEvent) => {
  if (isResizingSFTP.value) {
    sftpHeight.value = Math.max(100, Math.min(600, sftpHeight.value + e.movementY));
  }
};

// v2.10.3: Privilege Menu
const showPrivilegeMenu = ref(false);
const privilegeModule = ref('');
const privilegeMenuX = ref(0);
const privilegeMenuY = ref(0);

const onHeaderContextMenu = (p: { event: MouseEvent, module: string }) => {
  privilegeModule.value = p.module;
  privilegeMenuX.value = p.event.clientX;
  privilegeMenuY.value = p.event.clientY;
  showPrivilegeMenu.value = true;
};

const storageKey = (h: string) => `ter_tabs_${h.replace(/\s+/g, '_')}`;
let statsIntervalId: any = null;

// ==========================================
// --- DECOUPLED LOGIC ---
// ==========================================
const { 
  terminalTabs, activeTabId, activeTabIdSecondary, splitMode, toggleSplit, backgroundTabs, lastActivityMap,
  createNewTab, closeTab, sendToBackground, bringToForeground, renameTab 
} = useTabs(isConnected, backendLogs);

const {
  currentPath, realFiles, refreshExplorer, changeDir, onFastAccess
} = useExplorer(isConnected, activeTabId);

// v2.11.30: File Preview Logic
const showPreviewModal = ref(false);
const previewFileName = ref('');
const previewContent = ref('');

const closePreview = () => {
  showPreviewModal.value = false;
  previewContent.value = '';
};

const handlePreviewAction = async () => {
  if (selectedFile.value) {
    previewFileName.value = selectedFile.value.name;
    const content = await explorerActionPreview();
    if (content !== null) {
      previewContent.value = content;
      showPreviewModal.value = true;
    }
  }
};

const isClipFlashing = ref(false);
const copyLatestAI = async () => {
  if (!activeTabId.value) return;
  try {
    // v2.11.38: Call Native Backend API to bypass browser hand-shake restrictions
    await invoke('copy_latest_to_clipboard', { tabId: activeTabId.value });
    backendLogs.value.push(`[INFO] Latest AI response copied via Native API.`);
    
    // Flash effect
    isClipFlashing.value = true;
    setTimeout(() => isClipFlashing.value = false, 500);
  } catch (e) {
    backendLogs.value.push(`[ERROR] CLIP failed: ${e}`);
  }
};

const updateStatus = (msg: string) => {
  backendLogs.value.push(`[STATUS] ${msg}`);
};

// ...
const {
  showExplorerMenu, explorerMenuX, explorerMenuY, selectedFile,
  onExplorerContextMenu, explorerActionCd, explorerActionCat, explorerActionVim, explorerActionCopyPath, explorerActionRun,
  explorerActionDownload, explorerActionUpload, explorerActionDelete, explorerActionPreview,
  explorerActionDump, explorerActionWrite
} = useExplorerContextMenu(activeTabId, currentPath, refreshExplorer);

// v2.11.54: Quick Edit & Webview Integration
const handleQuickEdit = async () => {
  if (selectedFile.value) {
    const path = sanitizeSftpPath(selectedFile.value.path || (currentPath.value + '/' + selectedFile.value.name));
    try {
      const content = await invoke<string>('read_remote_file', { remotePath: path });
      await createNewTab(selectedFile.value.name, 'editor', { path, content });
    } catch (e) {
      backendLogs.value.push(`[ERROR] Failed to open editor: ${e}`);
    }
  }
  showExplorerMenu.value = false;
};

const onSaveComplete = () => {
  updateStatus("[FILE_SYNC_COMPLETE] Remote instance updated.");
};

const handleOpenInWebview = async () => {
  if (selectedFile.value) {
    const path = sanitizeSftpPath(selectedFile.value.path || (currentPath.value + '/' + selectedFile.value.name));
    const url = path.toLowerCase().endsWith('.pdf') ? `pdf://viewer?file=${encodeURIComponent(path)}` : `file://${path}`;
    await createNewTab(selectedFile.value.name, 'webview', { url, path });
  }
  showExplorerMenu.value = false;
};

const {
  webviewInstances, activeWebviewId, createWebview, closeWebview, switchWebview, updateWebviewUrl
} = useWebviews();

const {
  bookmarks, addBookmark, removeBookmark
} = useBookmarks(hostId);

const {
  uiPrefs, loadUIPreferences, autoDetectScale
} = useUIPreferences();

// v2.11.53: Debounced UI Scale for Terminal Performance
const debouncedUIScale = ref(uiPrefs.value.ui_scale);
let scaleDebounceTimer: any = null;

watch(() => uiPrefs.value.ui_scale, (newScale) => {
  if (scaleDebounceTimer) clearTimeout(scaleDebounceTimer);
  scaleDebounceTimer = setTimeout(() => {
    debouncedUIScale.value = newScale;
  }, 150); // 150ms debounce
});

const {
  previewUrl, isWebviewLoading, refreshWebview, handleExtractDOM, handleScrapeData, onDomExtracted, captureAndUpload, useNativeWebview, disableTunnel
} = useCyber(activeTabId, backendLogs, activeWebviewId, updateWebviewUrl);

// v2.11.43: Sync previewUrl when switching instances
watch(activeWebviewId, (newId) => {
  if (newId) {
    const inst = webviewInstances.value.find(w => w.id === newId);
    if (inst) previewUrl.value = inst.url;
  }
});

// v2.11.43: Web Context Menu State
const showWebMenu = ref(false);
const webMenuX = ref(0);
const webMenuY = ref(0);
const contextWebId = ref<string | null>(null);

const onWebContextMenu = (p: { event: MouseEvent, web: any }) => {
  webMenuX.value = p.event.clientX;
  webMenuY.value = p.event.clientY;
  contextWebId.value = p.web.id;
  showWebMenu.value = true;
};

// Handle event from Webview initialization script
listen('web-context-menu', (ev: any) => {
  const { x, y, id } = ev.payload;
  // Convert relative coordinates to absolute if necessary
  webMenuX.value = x; 
  webMenuY.value = y;
  contextWebId.value = id;
  showWebMenu.value = true;
});

const {
  showContextMenu, menuX, menuY, contextMenuTabId, hasErrorSelection,
  onTerminalContextMenu, copySelectedText, pasteFromClipboard, 
  renameTabAction, copyTabIdAction, copyRuntimeEnv, generateRunReport, diagnoseSelection, calculateMenuPosition
} = useContextMenu(activeTabId, renameTab, host, currentPath, currentAgentPort, terminalTabs);

const { 
  cpuChartRef, memChartRef, netChartRef, currentCpuUsage, 
  healthMode, currentNetSpeed, extraStats,
  initCharts, resizeCharts, fetchStats, setHealthMode
} = useStats(currentAgentPort, agentToken);

const cycleHealthMode = () => {
  const modes: any[] = ['resource', 'network', 'detail'];
  const next = modes[(modes.indexOf(healthMode.value) + 1) % modes.length];
  setHealthMode(next);
};

// v2.11.31: Sidebar Slot Logic
const sidebarSlots = ref(['OPS', 'ARS', 'NAV']);
const previousSlot3 = ref<string | null>(null);

const onAgentZoneClick = () => {
  // v2.11.52: Manual trigger for strategic logs
  if (sidebarSlots.value.includes('LOGS')) {
    window.dispatchEvent(new CustomEvent('switch-sidebar-view', { detail: 'LOGS' }));
  } else {
    // Overlay Protocol
    previousSlot3.value = sidebarSlots.value[2];
    sidebarSlots.value[2] = 'LOGS';
    nextTick(() => {
      window.dispatchEvent(new CustomEvent('switch-sidebar-view', { detail: 'LOGS' }));
    });
  }
};

const handleExplorerDownload = async () => {
  // v2.11.52: Force switch to NET logs for feedback
  onAgentZoneClick(); 
  nextTick(() => {
    const sidebar = document.querySelector('.side-bar');
    if (sidebar) {
      const netBtn = Array.from(sidebar.querySelectorAll('.tactical-logs-matrix button'))
        .find(b => b.textContent?.includes('NET')) as HTMLButtonElement;
      if (netBtn) netBtn.click();
    }
  });
  await explorerActionDownload(updateStatus);
};

const handleSidebarViewRevert = (newView: string) => {
  if (newView !== 'LOGS' && previousSlot3.value) {
    sidebarSlots.value[2] = previousSlot3.value;
    previousSlot3.value = null;
  }
};

const { 
  morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters,
  handleMorseMouse, handleMorseWheel, onMorseMacro
} = useMorse(activeTabId, calculateMenuPosition);

usePtyListener(
  activeTabId, connectionStatus, backendLogs, isAutoPilot, lastAutoPilotTime, 
  activeTriggers, captureAndUpload, refreshWebview, handleExtractDOM, currentAgentPort, lastActivityMap
);

// ==========================================
// --- WATCHERS ---
// ==========================================
watch(terminalTabs, (val) => { 
  if (isConnected.value) localStorage.setItem(storageKey(host.value), JSON.stringify(val)); 
}, { deep: true });

// ==========================================
// --- METHODS ---
// ==========================================
// RESTORED: Terminal Playback (Recording)
const viewHistory = async (originalTabId: string) => {
  const t = terminalTabs.value.find(x => x.id === originalTabId);
  const playbackId = await createNewTab(`Playback: ${t?.title || originalTabId}`, 'terminal', {}, true);
  try {
    const logs = await invoke<number[][]>('get_terminal_logs', { tabId: originalTabId, limit: 1000 });
    for (const chunk of logs) { 
      terminalManager.write(playbackId, new Uint8Array(chunk)); 
      await new Promise(r => setTimeout(r, 20)); 
    }
  } catch (e) { terminalManager.write(playbackId, `\r\n[ERROR] History Fail: ${e}\r\n`); }
};

const onConnected = async (data: { label: string, id: string }) => {
  const hostLabel = data.label;
  const hostIdValue = data.id;
  
  if (isConnected.value && host.value === hostLabel) return; // Prevent duplicate triggers
  
  host.value = hostLabel;
  activeServerId.value = hostIdValue;
  isConnected.value = true;
  connectionStatus.value = 'connected';
  
  // Create initial webview if none exists (v2.11.43)
  if (webviewInstances.value.length === 0) {
    createWebview('http://localhost:5173', 'Main Deck');
  }
  
  const saved = localStorage.getItem(storageKey(host.value));
  // Clear current tabs before restoring to prevent "shell explosion"
  terminalTabs.value = [];
  
  if (saved) {
    try {
      const ts = JSON.parse(saved); 
      if (Array.isArray(ts) && ts.length > 0) {
        // v2.12.1: Limit restoration to 5 tabs to prevent channel limit issues
        const restoreList = ts.slice(0, 5);
        for (const t of restoreList) {
          await createNewTab(t.title, 'terminal', {}, false, t.id);
        }
        activeTabId.value = restoreList.find((t: any) => !t.isBackground)?.id || restoreList[0]?.id;
      } else {
        await createNewTab("Main Shell", 'terminal', {}, false, "tab-1");
      }
    } catch (e) { 
      console.error("Restore failed:", e);
      await createNewTab("Main Shell", 'terminal', {}, false, "tab-1"); 
    }
  } else {
    await createNewTab("Main Shell", 'terminal', {}, false, "tab-1");
  }

  setTimeout(() => {
    refreshExplorer();
    invoke('load_remote_skills').then((s: any) => {
      skills.value = s;
    }).catch(() => {});
    nextTick(() => {
      initCharts();
      if (statsIntervalId) clearInterval(statsIntervalId);
      statsIntervalId = setInterval(fetchStats, 3000);
    });
  }, 1000);
};

// RESTORED: Skills & Macros
const runMacro = async (c: string) => { 
  if (activeTabId.value) await invoke('write_pty', { tabId: activeTabId.value, data: c + '\n' }); 
  showMorseMacro.value = false; 
};

const showSkillSettings = ref(false);
const selectedSkill = ref<any>(null);

const onSkillContextMenu = (p: { event: MouseEvent, skill: any }) => {
  selectedSkill.value = p.skill;
  showSkillSettings.value = true;
};

const runSkill = async (skill: any) => {
  if (!isConnected.value || !activeTabId.value) return;
  if (skill.context_file) {
    const f = skill.context_file;
    const fullPath = (currentPath.value === '/' ? '' : currentPath.value) + '/' + f.name;
    const cmd = `${skill.rpc || skill.trigger} "${fullPath}"\r\n`;
    invoke('write_pty', { tabId: activeTabId.value, data: cmd });
    return;
  }
  if (skill.context_requirement?.require_screenshot) await captureAndUpload(true);
  const rpc = skill.rpc || skill.trigger;
  if (rpc) invoke('write_pty', { tabId: activeTabId.value, data: rpc.endsWith('\n') ? rpc : rpc + "\r\n" });
};

const handleGlobalKeyDown = (e: KeyboardEvent) => { 
  if (e.ctrlKey) isCtrlPressed.value = true;
  if (e.altKey && e.key.toLowerCase() === 'l') isLocked.value = !isLocked.value; 
  if (e.ctrlKey && e.key.toLowerCase() === 't') {
    e.preventDefault();
    if (isConnected.value) createNewTab();
  }
};
onMounted(() => {
  loadUIPreferences();
  window.addEventListener('keydown', handleGlobalKeyDown);
  window.addEventListener('keyup', (e) => { if (!e.ctrlKey) isCtrlPressed.value = false; });
  window.addEventListener('mousemove', handleGlobalMouseMove);
  window.addEventListener('mouseup', stopResizingSFTP);
  
  // v2.11.29: Listen for custom event from xterm to close all menus
  window.addEventListener('close-all-menus', () => closeAllMenus());
  
  listen<string>('backend-log', (e) => { 
    if (!isLogsPaused.value) {
      logQueue.push(e.payload);
      if (!logThrottleId) {
        logThrottleId = setTimeout(processLogQueue, 100);
      }
    }
  });

  listen('net-traffic', () => {
    isTrafficFlashing.value = true;
    if (trafficTimeout) clearTimeout(trafficTimeout);
    trafficTimeout = setTimeout(() => isTrafficFlashing.value = false, 100);
  });
});

watch(isSafeMode, (val) => {
  localStorage.setItem('ter_safe_mode', val.toString());
  if (val) cyberMode.value = 0; // Force disable webviews in safe mode
});

// v2.11.18 FIX: Unified Menu Mutex (Moved to bottom to prevent ReferenceError)
const activeMenu = ref<string | null>(null);

const closeAllMenus = () => {
  activeMenu.value = null;
  showContextMenu.value = false;
  showExplorerMenu.value = false;
  showMorseMacro.value = false;
  showPrivilegeMenu.value = false;
  showWebMenu.value = false;
};

watch(activeMenu, (newVal) => {
  if (newVal === null) {
    showContextMenu.value = false;
    showExplorerMenu.value = false;
    showMorseMacro.value = false;
    showPrivilegeMenu.value = false;
    showWebMenu.value = false;
  }
});

watch(() => showContextMenu.value, (val) => { if (val) activeMenu.value = 'terminal'; });
watch(() => showExplorerMenu.value, (val) => { if (val) activeMenu.value = 'explorer'; });
watch(() => showMorseMacro.value, (val) => { if (val) activeMenu.value = 'morse'; });
watch(() => showPrivilegeMenu.value, (val) => { if (val) activeMenu.value = 'privilege'; });
watch(() => showWebMenu.value, (val) => { if (val) activeMenu.value = 'web'; });
</script>

<template>
  <div class="app-shell" 
       :class="{ 'safe-mode': isSafeMode }" 
       :style="{ 
         '--ter-ui-scale': uiPrefs.ui_scale,
         '--ter-glow-opacity': uiPrefs.glow_intensity / 100,
         '--ter-pulse-duration': (2.0 - (uiPrefs.pulse_speed / 100) * 1.8) + 's',
         'font-size': (14 * uiPrefs.ui_scale) + 'px'
       }"
       @click="closeAllMenus" @contextmenu="closeAllMenus">
    <CyberGate v-if="!isConnected" @connected="onConnected" />
    
    <div v-else class="main-view">
      <SettingsPanel :isOpen="showSettings" 
                     :useNativeWebview="useNativeWebview" 
                     :isSafeMode="isSafeMode"
                     :sidebarSlots="sidebarSlots"
                     :uiPrefs="uiPrefs"
                     @update:useNativeWebview="useNativeWebview = $event" 
                     @update:isSafeMode="isSafeMode = $event"
                     @update:sidebarSlots="sidebarSlots = $event"
                     @auto-detect="autoDetectScale"
                     @close="showSettings = false" @update-macros="(m) => activeMacros = m" />
      
      <SidebarPanel
        :class="{ 'collapsed': !isSidebarOpen }"
        :hostName="host"
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"        :webviewInstances="webviewInstances" :activeWebviewId="activeWebviewId"
        :lastActivityMap="lastActivityMap"
        :cpuChartRef="cpuChartRef" :memChartRef="memChartRef" :netChartRef="netChartRef"
        :healthMode="healthMode" :currentNetSpeed="currentNetSpeed" :extraStats="extraStats"
        :isAutoPilot="isAutoPilot"
        :isSafeMode="isSafeMode"
        :sftpHeight="sftpHeight"
        :slots="sidebarSlots"
        :isLogsOverlay="!!previousSlot3"
        :logs="backendLogs"
        @update:isAutoPilot="isAutoPilot = $event"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @explorer-context="onExplorerContextMenu" @cycle-health-mode="cycleHealthMode"
        @skill-context="onSkillContextMenu"
        @header-context="onHeaderContextMenu"
        @resize-sftp-start="startResizingSFTP"
        @resize-charts="resizeCharts"
        @view-changed="handleSidebarViewRevert"
        @switch-web="switchWebview"
        @web-context="onWebContextMenu"
        @open-vault-entry="(e) => createNewTab(e.name, 'editor', { path: e.path, content: e.content })"
      />

      <main class="workspace" @click.stop>
        <!-- ... existing modals ... -->

        <!-- Web Context Menu (v2.11.43) -->
        <div v-if="showWebMenu" class="context-menu" :style="{ top: webMenuY + 'px', left: webMenuX + 'px' }">
          <header class="menu-header">WEB ACTIONS</header>
          <div class="menu-item" @click="invoke('reload_cyber_webview', { label: contextWebId! }); activeMenu = null">🔄 Reload</div>
          <div class="menu-item" @click="createWebview(); activeMenu = null">➕ New Web Instance</div>
          <div class="menu-divider"></div>
          <div class="menu-item danger" @click="closeWebview(contextWebId!); activeMenu = null">❌ Close Page</div>
        </div>
        <!-- Skill Settings Modal -->
        <div v-if="showSkillSettings" class="modal-overlay" @click.self="showSkillSettings = false">
          <div class="auth-card cyber-card">
            <h2 class="cyber-title">SKILL_CONFIG: {{ selectedSkill?.name }}</h2>
            <div class="skill-form">
              <label class="label">RPC_COMMAND</label>
              <input v-model="selectedSkill.rpc" class="cyber-input" />
              <label class="label">DESCRIPTION</label>
              <textarea v-model="selectedSkill.description" class="cyber-input" rows="3"></textarea>
            </div>
            <button @click="showSkillSettings = false" class="btn-primary">APPLY</button>
          </div>
        </div>

        <!-- Terminal Context Menu -->
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename</div>
          <div class="menu-item" @click="sendToBackground(contextMenuTabId!)">🚀 Background</div>
          <div class="menu-divider"></div>
          <div class="menu-item" @click="copySelectedText">📋 Copy</div>
          <div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div>
          <header class="menu-header">TMUX CONTROL</header>
          <div class="menu-item" @click="invoke('write_pty', { tabId: activeTabId, data: '\x02%' }); activeMenu = null">◫ Split Horizontal</div>
          <div class="menu-item" @click="invoke('write_pty', { tabId: activeTabId, data: '\x02\&quot;' }); activeMenu = null">⬒ Split Vertical</div>
          <div class="menu-item" @click="invoke('write_pty', { tabId: activeTabId, data: '\x02z' }); activeMenu = null">⤢ Toggle Zoom</div>
          <div class="menu-item danger" @click="invoke('write_pty', { tabId: activeTabId, data: '\x02x' }); activeMenu = null">✕ Kill Pane</div>
          <div class="menu-divider"></div>
          <div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Close</div>
        </div>

        <!-- Explorer Menu -->
        <div v-if="showExplorerMenu" class="context-menu" :style="{ top: explorerMenuY + 'px', left: explorerMenuX + 'px' }">
          <header class="menu-header">FILE ACTIONS</header>
          <template v-if="selectedFile?.is_dir">
            <div class="menu-item" @click="explorerActionCd">📂 Open Folder</div>
            <div class="menu-item" @click="explorerActionUpload">📤 Upload</div>
          </template>
          <template v-else>
            <div class="menu-item highlight" @click="handleOpenInWebview">👁️ OPEN_IN_WEBVIEW</div>
            <div class="menu-item" @click="explorerActionDump">📟 DUMP_TO_TERMINAL</div>
            <div class="menu-item" @click="handleQuickEdit">📝 QUICK_EDIT</div>
            <div class="menu-divider"></div>
            <div class="menu-item" @click="handleExplorerDownload">📥 Download</div>
            <div class="menu-item" @click="handlePreviewAction">👁️ Preview</div>
            <div class="menu-divider"></div>
            <div class="menu-item danger" @click="explorerActionDelete">🗑️ Delete</div>
          </template>
        </div>

        <!-- File Preview Modal -->
        <div v-if="showPreviewModal" class="modal-overlay preview-overlay" @click.self="closePreview">
          <div class="preview-card cyber-card">
            <header class="preview-header">
              <span class="title">👁️ PREVIEWING: {{ previewFileName }}</span>
              <button class="close-btn" @click="closePreview">✕</button>
            </header>
            <div class="preview-body scroller">
              <pre class="preview-text">{{ previewContent }}</pre>
            </div>
          </div>
        </div>

        <!-- Morse Macros -->
        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
        </div>

        <!-- Privilege Menu -->
        <div v-if="showPrivilegeMenu" class="context-menu" :style="{ top: privilegeMenuY + 'px', left: privilegeMenuX + 'px' }">
          <header class="menu-header">CYBER PRIVILEGE: {{ privilegeModule.toUpperCase() }}</header>
          <div class="menu-item">🛠️ Deep Diagnostic</div>
          <div class="menu-item">🛡️ Secure Isolation</div>
          <div class="menu-item highlight">☢️ Core Override</div>
        </div>

        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs 
              :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" 
              :isMorsePressed="isMorsePressed" :morseSequence="morseSequence"
              :uiScale="debouncedUIScale"
              :activeTabIdSecondary="activeTabIdSecondary"
              :splitMode="splitMode"
              @switch-tab="bringToForeground" 
              @switch-tab-secondary="(id) => activeTabIdSecondary = id"
              @toggle-split="toggleSplit"
              @save-complete="onSaveComplete"
              @close-tab="closeTab" @new-tab="createNewTab()" 
              @terminal-context="onTerminalContextMenu" 
              @morse-input="handleMorseMouse"
            />
          </section>
          <section class="cyber-pane" :class="{ 'open': cyberMode === 1 }">
            <div class="cyber-container">
              <div class="cyber-logs-view">
                <header><span class="title">Cyber Logs (HUD)</span></header>
                <div class="logs-container">
                  <div v-for="(log, i) in tacticalLogs" :key="i" class="log-line" :style="{ color: getLogColor(log) }">
                    {{ log }}
                  </div>
                </div>
              </div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <div class="engine-indicator" :class="{ 'native': useNativeWebview }">
                    {{ useNativeWebview ? '⚡ Native' : '🐢 Iframe' }}
                  </div>
                  <input v-model="previewUrl" @keyup.enter="refreshWebview(previewUrl)" class="address-bar-input" />
                  <button @click="refreshWebview(previewUrl)" class="refresh-btn">⚡</button>
                  <button @click="handleScrapeData()" class="refresh-btn" title="Scrape Page Content (h3)">📊</button>
                  <button @click="disableTunnel = !disableTunnel" class="refresh-btn" :title="disableTunnel ? 'Enable Remote Tunnel' : 'Disable Remote Tunnel'" :style="{ color: disableTunnel ? '#ef4444' : '#22c55e' }">                    <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 11V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v5"></path><rect x="5" y="11" width="14" height="10" rx="2"></rect><circle cx="12" cy="16" r="1"></circle></svg>
                  </button>
                  <button @click="addBookmark(activeWebviewId || 'Web', previewUrl)" class="refresh-btn">🔖</button>
                </nav>
                
                <!-- v2.11.43: Bookmarks Bar -->
                <div class="bookmarks-bar" v-if="bookmarks.length > 0">
                  <div v-for="b in bookmarks" :key="b.id" class="bookmark-item" @click="refreshWebview(b.url)" @contextmenu.prevent="removeBookmark(b.id)">
                    {{ b.title }}
                  </div>
                </div>

                <div class="webview-container" style="flex: 1; display: flex; flex-direction: column; height: 100%; background: #000;">
                   <!-- Multi-Instance Renderer -->
                   <template v-if="cyberMode === 1 && useNativeWebview && !isSafeMode">
                     <CyberWebview 
                       v-for="inst in webviewInstances" 
                       :key="inst.id"
                       v-show="activeWebviewId === inst.id"
                       :id="inst.id"
                       :url="inst.url"
                       :isActive="activeWebviewId === inst.id"
                       :isSafeMode="isSafeMode"
                       @dom-extracted="onDomExtracted"
                     />
                   </template>
                   <div v-else-if="isSafeMode" class="safe-mode-placeholder">
                     <span class="icon">🛡️</span>
                     <div class="msg">WEB_ENGINE_DISABLED_IN_SAFE_MODE</div>
                     <button class="os-browser-btn" @click="isSafeMode = false">DISABLE_SAFE_MODE</button>
                   </div>
                   <iframe v-else-if="cyberMode === 1 && !useNativeWebview" :src="previewUrl" class="cyber-iframe" frameborder="0" style="flex: 1; width: 100%; height: 100%; background: #ffffff;"></iframe>
                </div>
              </div>
            </div>
          </section>
        </div>

        <footer class="status-bar">
          <div class="status-left">
            <button class="status-btn sidebar-toggle" @click.stop="isSidebarOpen = !isSidebarOpen">
              {{ isSidebarOpen ? 'SIDE_HIDE' : 'SIDE_SHOW' }}
            </button>
            <span class="status-sep">|</span>
            <div class="status-item node-info" @click="showNetworkMatrix = true">
              NODE: {{ host }}
            </div>
            <span class="status-sep">|</span>
            <div class="status-item traffic-indicator" :class="{ 'flashing': isTrafficFlashing }">
              NET_TRAFFIC
            </div>
            <span class="status-sep">|</span>
            <div class="status-item agent-zone" 
                 :class="{ 'active': isConnected }"
                 @click="onAgentZoneClick"
                 @contextmenu.prevent>
              AGENT: {{ isConnected ? 'ACTIVE' : 'OFFLINE' }}
            </div>
          </div>

          <div class="hotkey-bar">
            <button class="status-btn modifier" @click="invoke('write_pty', { tabId: activeTabId, data: '\t' })">TAB</button>
            <button class="status-btn modifier" :class="{ 'active': isCtrlPressed }" @click="isCtrlPressed = !isCtrlPressed">CTRL</button>
            <button class="status-btn modifier" @click="invoke('write_pty', { tabId: activeTabId, data: '\x03' })">C-C</button>
            <button class="status-btn modifier" @click="invoke('write_pty', { tabId: activeTabId, data: '\x1b' })">ESC</button>
          </div>

          <div class="status-right">
            <button class="status-btn" :class="{ 'active': isSafeMode }" @click="isSafeMode = !isSafeMode">
              🛡️ {{ isSafeMode ? 'SAFE_MODE: ON' : 'SAFE_MODE: OFF' }}
            </button>
            <span class="status-sep">|</span>
            <button class="status-btn" :class="{ 'clip-flash': isClipFlashing }" @click="copyLatestAI">📋 CLIP</button>
            <span class="status-sep">|</span>
            <button class="status-btn" @click="captureAndUpload(false)">AUDIT_UI</button>
            <span class="status-sep">|</span>
            <button class="status-btn web-toggle" :class="{ 'active': cyberMode === 1 }" @click="cyberMode = cyberMode === 1 ? 0 : 1">
              WEB_ENGINE: {{ cyberMode === 1 ? 'ON' : 'OFF' }}
            </button>
            <span class="status-sep">|</span>
            <button class="status-btn auto-toggle" :class="{ 'active': isAutoPilot }" @click="isAutoPilot = !isAutoPilot">
              AUTO_SYNC: {{ isAutoPilot ? 'ON' : 'OFF' }}
            </button>
            <span class="status-sep">|</span>
            <button class="status-btn lock-btn" :class="{ 'active': isLocked }" @click="isLocked = true">SYS_LOCK</button>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
    <NetworkMatrix v-if="showNetworkMatrix" :activeId="activeServerId" :activeTabId="activeTabId" @close="showNetworkMatrix = false" />
  </div>
</template>

<style scoped>
.app-shell { 
  height: 100vh; 
  width: 100vw; 
  background: #000; 
  color: #d4d4d8; 
  font-family: 'Inter', 'Segoe UI', 'Ubuntu', 'Noto Sans', 'Cantarell', 'Helvetica Neue', system-ui, sans-serif;
  overflow: hidden; 
  display: flex; 
  flex-direction: column; 
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
}

.app-shell :deep(*) {
  font-family: 'Inter', 'Segoe UI', 'Ubuntu', 'Noto Sans', 'Cantarell', 'Helvetica Neue', system-ui, sans-serif;
}

/* Maintain monospace for specific technical elements */
.app-shell :deep(.terminal-pane *), 
.app-shell :deep(.logs-container *), 
.app-shell :deep(.kb-pendant),
.app-shell :deep(.branding-text),
.app-shell :deep(.cyber-input),
.app-shell :deep(.address-bar-input) {
  font-family: 'JetBrains Mono', 'Ubuntu Mono', 'DejaVu Sans Mono', 'Liberation Mono', 'Fira Code', 'Courier New', monospace !important;
}

.main-view { display: flex; flex: 1; height: 100%; width: 100%; overflow: hidden; position: relative; }

/* FIX: Ensure side-bar completely vanishes when collapsed */
:deep(.side-bar) {
  width: var(--ter-sidebar-width);
  flex-shrink: 0;
  transition: width 0.2s cubic-bezier(0.4, 0, 0.2, 1), padding 0.2s ease, opacity 0.2s ease;
  overflow: hidden;
}

.app-shell :deep(.module) {
  border-radius: calc(6px * var(--ter-ui-scale));
  overflow: hidden;
}

:deep(.side-bar.collapsed) {
  width: 0 !important;
  min-width: 0 !important;
  padding: 0 !important;
  margin: 0 !important;
  border: none !important;
  opacity: 0;
  pointer-events: none;
}

.workspace { 
  flex: 1; 
  display: flex; 
  flex-direction: column; 
  background: #000; 
  overflow: hidden; 
  position: relative; 
  height: 100%;
  min-width: 0; /* CRITICAL: Allows terminal to expand properly */
  width: 100%;
}
.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; width: 100%; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; display: flex; flex-direction: column; overflow: hidden; background: #000; }
.cyber-pane { width: calc(420px * var(--ter-ui-scale)); height: 100%; border-left: 1px solid #27272a; display: none; flex-direction: column; background: #000; }
.cyber-pane.open { display: flex; }
.cyber-container { display: flex; flex-direction: column; height: 100%; }
.cyber-logs-view { flex: 0 0 30%; border-bottom: 1px solid #27272a; overflow: hidden; display: flex; flex-direction: column; }
.cyber-logs-view header { padding: calc(5px * var(--ter-ui-scale)) calc(100px * var(--ter-ui-scale)); font-size: calc(11px * var(--ter-ui-scale)); color: #71717a; border-bottom: 1px solid #18181b; letter-spacing: 0.5px; }
.logs-container { flex: 1; overflow-y: auto; padding: calc(10px * var(--ter-ui-scale)); font-size: calc(11px * var(--ter-ui-scale)); color: #a1a1aa; }
.cyber-webview-wrapper { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { padding: calc(5px * var(--ter-ui-scale)); background: #09090b; border-bottom: 1px solid #27272a; display: flex; gap: calc(5px * var(--ter-ui-scale)); }
.address-bar-input { flex: 1; background: #000; border: 1px solid #27272a; color: #22c55e; padding: 2px 8px; font-size: calc(11px * var(--ter-ui-scale)); outline: none; border-radius: 4px; }
.refresh-btn { background: #18181b; border: 1px solid #27272a; color: #22c55e; cursor: pointer; padding: 0 calc(8px * var(--ter-ui-scale)); border-radius: 4px; }

/* v2.11.43: Bookmarks Bar Styles */
.bookmarks-bar { display: flex; gap: calc(8px * var(--ter-ui-scale)); padding: calc(4px * var(--ter-ui-scale)) calc(8px * var(--ter-ui-scale)); background: #000; border-bottom: 1px solid #18181b; overflow-x: auto; scrollbar-width: none; }
.bookmarks-bar::-webkit-scrollbar { display: none; }
.bookmark-item { font-size: calc(9px * var(--ter-ui-scale)); color: #71717a; padding: 2px 8px; border: 1px solid #27272a; border-radius: 4px; cursor: pointer; white-space: nowrap; transition: all 0.2s; }
.bookmark-item:hover { color: #22c55e; border-color: #22c55e; background: rgba(34, 197, 94, 0.05); }

.safe-mode-placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; background: #09090b; color: #71717a; gap: calc(15px * var(--ter-ui-scale)); font-family: 'JetBrains Mono', monospace; }
.safe-mode-placeholder .icon { font-size: calc(32px * var(--ter-ui-scale)); }
.safe-mode-placeholder .msg { font-size: calc(12px * var(--ter-ui-scale)); letter-spacing: 1px; }
.safe-mode-placeholder .os-browser-btn { background: #18181b; border: 1px solid #27272a; color: #22c55e; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-size: calc(11px * var(--ter-ui-scale)); }

.engine-indicator { 
  font-size: calc(9px * var(--ter-ui-scale)); 
  padding: 2px 6px; 
  border-radius: 4px; 
  background: #18181b; 
  color: #71717a; 
  border: 1px solid #27272a;
  white-space: nowrap;
  display: flex;
  align-items: center;
}
.engine-indicator.native { color: #a855f7; border-color: rgba(168, 85, 247, 0.4); }

.clip-flash {
  color: #00ff9d !important;
  text-shadow: 0 0 calc(8px * var(--ter-ui-scale)) #00ff9d !important;
  transform: scale(1.1);
}

.status-bar { 
  height: var(--ter-status-bar-height); 
  background: #09090b; 
  border-top: 1px solid #18181b; 
  display: flex; 
  justify-content: space-between; 
  align-items: center; 
  padding: 0 12px; 
  font-size: calc(11px * var(--ter-ui-scale)); 
  flex-shrink: 0; 
  z-index: 1000;
  text-transform: uppercase;
  letter-spacing: 1px;
}

.status-left, .status-right, .hotkey-bar { display: flex; align-items: center; gap: 8px; }
.status-sep { color: #27272a; font-size: 10px; margin: 0 4px; pointer-events: none; }

.traffic-indicator { font-size: 9px; color: #52525b; transition: all 0.1s; font-family: 'JetBrains Mono', monospace; }
.traffic-indicator.flashing { color: #22c55e; text-shadow: 0 0 5px #22c55e; }

.status-btn { 
  background: transparent; 
  border: none; 
  color: #52525b; 
  cursor: pointer; 
  padding: 4px 8px; 
  font-family: 'JetBrains Mono', monospace !important;
  font-size: 11px;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  display: flex;
  align-items: center;
  border-radius: 4px;
}

.status-btn:hover { 
  color: #fff; 
  transform: scale(1.05);
}

.status-btn.active { color: #fff; text-shadow: 0 0 4px currentColor; animation: breathe 2s infinite ease-in-out; }

/* Theme Colors */
.agent-zone.active { color: #22c55e; text-shadow: 0 0 5px rgba(34, 197, 94, 0.5); animation: breathe 2s infinite ease-in-out; }
.agent-zone.pressing { transform: scale(0.95); filter: brightness(1.5); }
.agent-zone { position: relative; cursor: crosshair; padding: 4px 8px; transition: all 0.1s; }

.morse-preview {
  position: absolute;
  top: -18px;
  left: 50%;
  transform: translateX(-50%);
  background: #22c55e;
  color: #000;
  padding: 1px 4px;
  border-radius: 2px;
  font-size: 9px;
  font-weight: bold;
  opacity: 0;
  transition: opacity 0.2s;
  pointer-events: none;
}
.agent-zone.pressing .morse-preview, .agent-zone:hover .morse-preview { opacity: 1; }

.web-toggle.active { color: #3b82f6; text-shadow: 0 0 5px rgba(59, 130, 246, 0.5); }
.auto-toggle.active { color: #a855f7; text-shadow: 0 0 5px rgba(168, 85, 247, 0.5); }
.modifier.active { color: #a855f7; background: rgba(168, 85, 247, 0.1); border: 1px solid rgba(168, 85, 247, 0.2); }
.lock-btn:hover { color: #ef4444 !important; text-shadow: 0 0 5px rgba(239, 68, 68, 0.5); }

@keyframes breathe {
  0%, 100% { opacity: 1; filter: brightness(1); }
  50% { opacity: 0.7; filter: brightness(1.3); }
}

.node-info { cursor: pointer; color: #71717a; transition: color 0.2s; }
.node-info:hover { color: #a855f7; }

/* Remove old status bar styles */
.status-divider, .node-dot, .tiny-dot, .kb-pendant { display: none; }

.context-menu { 
  position: fixed !important; 
  z-index: 99999 !important; 
  background: rgba(9, 9, 11, 0.95) !important; 
  backdrop-filter: blur(10px);
  border: 1px solid #22c55e !important; 
  padding: 10px !important; 
  box-shadow: 0 0 10px #22c55e !important; 
  border-radius: 6px !important; 
}
.menu-header { padding: 4px 8px; font-size: 10px; color: #166534; border-bottom: 1px solid #18181b; margin-bottom: 4px; letter-spacing: 0.5px; }
.menu-item { padding: 6px 12px; font-size: 12px; cursor: pointer; color: #d4d4d8; border-radius: 4px; margin-bottom: 1px; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-item.danger { color: #ef4444; }
.menu-item.danger:hover { background: #ef4444; color: #000; }
.menu-divider { height: 1px; background: #18181b; margin: 4px 0; }

.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.85); backdrop-filter: blur(5px); display: flex; align-items: center; justify-content: center; z-index: 20000; }
.cyber-card { background: #09090b; border: 1px solid #22c55e; padding: 30px; min-width: 400px; box-shadow: 0 0 30px rgba(34, 197, 94, 0.2); border-radius: 8px; }
.cyber-title { color: #22c55e; font-size: 18px; letter-spacing: 2px; margin-bottom: 15px; }
.skill-form { display: flex; flex-direction: column; gap: 10px; }
.label { font-size: 10px; color: #71717a; text-transform: uppercase; }
.cyber-input { background: #000; border: 1px solid #27272a; color: #22c55e; padding: 8px; font-size: 12px; outline: none; width: 100%; border-radius: 4px; }
.btn-primary { background: #22c55e; color: #000; border: none; padding: 10px; font-weight: bold; cursor: pointer; margin-top: 15px; border-radius: 4px; }

/* v2.11.30: Preview Modal Styles */
.preview-overlay { z-index: 30000; background: rgba(0, 0, 0, 0.9); backdrop-filter: blur(15px); }
.preview-card { width: 80vw; height: 80vh; max-width: 1000px; display: flex; flex-direction: column; padding: 0; overflow: hidden; border-color: #3b82f6; box-shadow: 0 0 40px rgba(59, 130, 246, 0.2); }
.preview-header { display: flex; justify-content: space-between; align-items: center; padding: 15px 20px; background: rgba(59, 130, 246, 0.1); border-bottom: 1px solid rgba(59, 130, 246, 0.2); }
.preview-header .title { font-size: 12px; font-family: 'JetBrains Mono', monospace; color: #3b82f6; letter-spacing: 1px; }
.preview-header .close-btn { background: transparent; border: none; color: #71717a; cursor: pointer; font-size: 18px; }
.preview-header .close-btn:hover { color: #fff; }
.preview-body { flex: 1; padding: 20px; overflow: auto; background: #000; }
.preview-text { margin: 0; font-family: 'JetBrains Mono', monospace; font-size: 13px; color: #d4d4d8; line-height: 1.6; white-space: pre-wrap; word-break: break-all; }

.mini-switch { position: relative; display: inline-block; width: 24px; height: 12px; }
.mini-switch input { opacity: 0; width: 0; height: 0; }
.slider { position: absolute; cursor: pointer; inset: 0; background-color: #27272a; transition: .4s; border-radius: 12px; }
.slider:before { position: absolute; content: ""; height: 8px; width: 8px; left: 2px; bottom: 2px; background-color: white; transition: .4s; border-radius: 50%; }
input:checked + .slider { background-color: #3b82f6; }
input:checked + .slider:before { transform: translateX(12px); }
.status-toggle { display: flex; align-items: center; gap: 8px; font-size: 11px; color: #52525b; }

.kb-pendant { background: rgba(39, 39, 42, 0.3); border: 0.5px solid rgba(255,255,255,0.05); color: #71717a; font-size: 10px; padding: 2px 8px; border-radius: 4px; cursor: pointer; transition: all 0.2s; }
.kb-pendant.active { background: #22c55e; color: #000; box-shadow: 0 0 10px #22c55e; }
.hotkey-bar { display: flex; gap: 5px; }

/* Higher specificity overrides for existing components */
.app-shell :deep(.tab-item) { font-size: 12px; letter-spacing: 0.5px; border-radius: 4px 4px 0 0; }
.app-shell :deep(.name), .app-shell :deep(.file-name) { font-size: 12px; }
.app-shell :deep(.header-with-action) { font-size: 11px; letter-spacing: 0.5px; }
.app-shell :deep(.stealth-zone) { 
  border-radius: 4px; 
  pointer-events: auto !important; 
  cursor: crosshair !important;
  z-index: 99999 !important;
}
.app-shell.safe-mode :deep(*) {
  text-shadow: none !important;
  box-shadow: none !important;
  animation: none !important;
  backdrop-filter: none !important;
  transition: none !important;
}
.app-shell.safe-mode :deep(.scanline) { display: none !important; }
</style>
