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
import MatrixAllocator from './components/MatrixAllocator.vue';

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

// v2.14.0: TER_CORE State Center Integration
import { 
  globalState, backendLogs, terminalTabs, activeTabId, activeTabIdSecondary, 
  splitMode, webviewInstances, activeWebviewId, hostId, storeActions 
} from './store';

// ==========================================
// --- GLOBAL STATE (REFACTORED) ---
// ==========================================
const isAutoPilot = ref(false);
const lastAutoPilotTime = ref(0);
const activeTriggers = ref<string[]>(['Allow execution of:', '1. Allow once']);
const activeMacros = ref<{name: string, cmd: string}[]>([]);

const isLogsPaused = ref(false);
const skills = ref<any[]>([]);

const tacticalLogs = computed(() => backendLogs.value.slice(-50));
const isTrafficFlashing = ref(false);
let trafficTimeout: any = null;

const getLogColor = (log: string) => {
  if (log.includes('[ERROR]')) return '#ef4444';
  if (log.includes('[SYSTEM]') || log.includes('[STATUS]')) return '#22c55e';
  if (log.includes('[DEBUG]') || log.includes('[INFO]')) return '#888888';
  if (log.includes('AI') || log.includes('Reasoning')) return '#a855f7';
  return '#a1a1aa';
};

const isResizingSFTP = ref(false);

const handleResizeSFTP = (newHeight: number) => {
  globalState.sftpHeight = Math.max(100, Math.min(600, newHeight));
};

const handleGlobalMouseMove = (e: MouseEvent) => {
  if (isResizingSFTP.value) {
    globalState.sftpHeight = Math.max(100, Math.min(600, globalState.sftpHeight + e.movementY));
  }
};

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

const sharedProps = computed(() => ({
  // Sidebar Props
  hostName: globalState.host,
  bgTabs: backgroundTabs.value,
  skills: skills.value,
  webviewInstances: webviewInstances.value,
  activeWebviewId: activeWebviewId.value,
  lastActivityMap: lastActivityMap.value,
  cpuChartRef: cpuChartRef.value,
  memChartRef: memChartRef.value,
  netChartRef: netChartRef.value,
  healthMode: healthMode.value,
  currentNetSpeed: currentNetSpeed.value,
  extraStats: extraStats.value,
  isAutoPilot: isAutoPilot.value,
  isSafeMode: globalState.isSafeMode,
  sftpHeight: globalState.sftpHeight,
  slots: sidebarSlots.value,
  isLogsOverlay: !!previousSlot3.value,
  
  // Terminal Props
  tabs: terminalTabs.value,
  activeTabId: activeTabId.value,
  connectionStatus: globalState.connectionStatus,
  isMorsePressed: isMorsePressed.value,
  morseSequence: morseSequence.value,
  uiScale: debouncedUIScale.value,
  activeTabIdSecondary: activeTabIdSecondary.value,
  splitMode: splitMode.value,
  isSidebarOpen: globalState.isSidebarOpen,
}));

const handleUpdateLayout = (layoutType: string) => {
  if (layoutType === 'classic') {
    globalState.workspaceMatrix = {
      version: 1, zoneLeft: 'SIDEBAR_PANEL', zoneMain: 'TERMINAL_MAIN', zoneRight: 'NONE', leftRatio: 25, rightRatio: 25
    };
  } else if (layoutType === 'developer') {
    globalState.workspaceMatrix = {
      version: 1, zoneLeft: 'SIDEBAR_PANEL', zoneMain: 'TERMINAL_MAIN', zoneRight: 'CYBER_HUD', leftRatio: 20, rightRatio: 30
    };
  } else if (layoutType === 'ops') {
    globalState.workspaceMatrix = {
      version: 1, zoneLeft: 'SIDEBAR_PANEL', zoneMain: 'TERMINAL_MAIN', zoneRight: 'SFTP_EXPLORER', leftRatio: 20, rightRatio: 20
    };
  }
  localStorage.setItem('ter_matrix', JSON.stringify(globalState.workspaceMatrix));
};

// ==========================================
// --- DECOUPLED LOGIC (UPDATED) ---
// ==========================================
const { 
  backgroundTabs, lastActivityMap,
  createNewTab, closeTab, sendToBackground, bringToForeground, renameTab 
} = useTabs();

const {
  realFiles, refreshExplorer, changeDir, onFastAccess
} = useExplorer();

const showPreviewModal = ref(false);
const previewFileName = ref('');
const previewContent = ref('');

const closePreview = () => {
  showPreviewModal.value = false;
  previewContent.value = '';
};

const handleVaultEntry = (entry: any) => {
  previewFileName.value = entry.title || 'Vault Entry';
  previewContent.value = entry.content || JSON.stringify(entry, null, 2);
  showPreviewModal.value = true;
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
    await invoke('copy_latest_to_clipboard', { tabId: activeTabId.value });
    backendLogs.value.push(`[INFO] Latest AI response copied via Native API.`);
    isClipFlashing.value = true;
    setTimeout(() => isClipFlashing.value = false, 500);
  } catch (e) {
    backendLogs.value.push(`[ERROR] CLIP failed: ${e}`);
  }
};

const updateStatus = (msg: string) => {
  backendLogs.value.push(`[STATUS] ${msg}`);
};

const {
  createWebview, closeWebview, switchWebview, updateWebviewUrl
} = useWebviews();

const {
  bookmarks, addBookmark, removeBookmark
} = useBookmarks(hostId);

const {
  uiPrefs, loadUIPreferences, autoDetectScale
} = useUIPreferences();

const debouncedUIScale = ref(uiPrefs.value.ui_scale);
let scaleDebounceTimer: any = null;

watch(() => uiPrefs.value.ui_scale, (newScale) => {
  if (scaleDebounceTimer) clearTimeout(scaleDebounceTimer);
  scaleDebounceTimer = setTimeout(() => {
    debouncedUIScale.value = newScale;
  }, 150);
});

const {
  previewUrl, isWebviewLoading, refreshWebview, handleExtractDOM, handleScrapeData, onDomExtracted, captureAndUpload, useNativeWebview, disableTunnel
} = useCyber(activeTabId, backendLogs, activeWebviewId, updateWebviewUrl);

watch(activeWebviewId, (newId) => {
  if (newId) {
    const inst = webviewInstances.value.find(w => w.id === newId);
    if (inst) previewUrl.value = inst.url;
  }
});

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

listen('web-context-menu', (ev: any) => {
  const { x, y, id } = ev.payload;
  webMenuX.value = x; 
  webMenuY.value = y;
  contextWebId.value = id;
  showWebMenu.value = true;
});

const {
  showContextMenu, menuX, menuY, contextMenuTabId, hasErrorSelection,
  onTerminalContextMenu, copySelectedText, pasteFromClipboard, 
  renameTabAction, copyTabIdAction, copyRuntimeEnv, generateRunReport, diagnoseSelection, calculateMenuPosition
} = useContextMenu(activeTabId, renameTab, computed(() => globalState.host), computed(() => globalState.currentPath), computed(() => globalState.currentAgentPort), terminalTabs);

const { 
  cpuChartRef, memChartRef, netChartRef, currentCpuUsage, 
  healthMode, currentNetSpeed, extraStats,
  initCharts, resizeCharts, fetchStats, setHealthMode
} = useStats(computed(() => globalState.currentAgentPort), computed(() => globalState.agentToken));

const { 
  morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters,
  handleMorseMouse, handleMorseWheel, onMorseMacro
} = useMorse(activeTabId, calculateMenuPosition);

usePtyListener(
  isAutoPilot, lastAutoPilotTime, 
  activeTriggers, captureAndUpload, refreshWebview, handleExtractDOM, lastActivityMap
);

const {
  showExplorerMenu, explorerMenuX, explorerMenuY, selectedFile,
  onExplorerContextMenu, explorerActionCd, explorerActionCat, explorerActionVim, explorerActionCopyPath, explorerActionRun,
  explorerActionDownload, explorerActionUpload, explorerActionDelete, explorerActionPreview,
  explorerActionDump, explorerActionWrite
} = useExplorerContextMenu(activeTabId, computed(() => globalState.currentPath), refreshExplorer);

const handleQuickEdit = async () => {
  if (selectedFile.value) {
    const path = sanitizeSftpPath(selectedFile.value.path || (globalState.currentPath + '/' + selectedFile.value.name));
    try {
      const content = await invoke<string>('read_remote_file', { remotePath: path });
      await createNewTab(selectedFile.value.name, 'editor', { path, content });
    } catch (e) {
      backendLogs.value.push(`[ERROR] Failed to open editor: ${e}`);
    }
  }
  showExplorerMenu.value = false;
};

const onSaveComplete = () => updateStatus("[FILE_SYNC_COMPLETE] Remote instance updated.");

const handleOpenInWebview = async () => {
  if (selectedFile.value) {
    const path = sanitizeSftpPath(selectedFile.value.path || (globalState.currentPath + '/' + selectedFile.value.name));
    const url = path.toLowerCase().endsWith('.pdf') ? `pdf://viewer?file=${encodeURIComponent(path)}` : `file://${path}`;
    await createNewTab(selectedFile.value.name, 'webview', { url, path });
  }
  showExplorerMenu.value = false;
};

const handleExplorerDownload = async () => {
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

const cycleHealthMode = () => {
  const modes: any[] = ['resource', 'network', 'detail'];
  const next = modes[(modes.indexOf(healthMode.value) + 1) % modes.length];
  setHealthMode(next);
};

const sidebarSlots = ref(['OPS', 'ARS', 'NAV']);
const previousSlot3 = ref<string | null>(null);

const onAgentZoneClick = () => {
  if (sidebarSlots.value.includes('LOGS')) {
    window.dispatchEvent(new CustomEvent('switch-sidebar-view', { detail: 'LOGS' }));
  } else {
    previousSlot3.value = sidebarSlots.value[2];
    sidebarSlots.value[2] = 'LOGS';
    window.dispatchEvent(new CustomEvent('switch-sidebar-view', { detail: 'LOGS' }));
  }
};

const handleSidebarViewRevert = (newView: string) => {
  if (newView !== 'LOGS' && previousSlot3.value) {
    sidebarSlots.value[2] = previousSlot3.value;
    previousSlot3.value = null;
  }
};

const onConnected = async (data: { label: string, id: string }) => {
  const hostLabel = data.label;
  const hostIdValue = data.id;
  
  if (globalState.isConnected && globalState.host === hostLabel) return; 
  
  storeActions.setConnected(true, hostLabel, hostIdValue);
  
  if (webviewInstances.value.length === 0) {
    createWebview('http://localhost:5173', 'Main Deck');
  }
  
  const saved = localStorage.getItem(storageKey(globalState.host));
  terminalTabs.value = [];
  
  if (saved) {
    try {
      const ts = JSON.parse(saved); 
      if (Array.isArray(ts) && ts.length > 0) {
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
      skills.value = Array.isArray(s) ? s : [];
    });
  }, 1000);

  if (statsIntervalId) clearInterval(statsIntervalId);
  statsIntervalId = setInterval(fetchStats, 2000);
};

const runMacro = async (cmd: string) => {
  if (activeTabId.value) {
    await invoke('write_pty', { tabId: activeTabId.value, data: cmd + '\r' });
    activeMenu.value = null;
  }
};

const runSkill = async (skill: any) => {
  const tid = activeTabId.value;
  if (!tid) return;
  const ctx = skill.context_file ? ` --context "${skill.context_file.path}"` : "";
  await invoke('write_pty', { tabId: tid, data: `${skill.rpc}${ctx}\r` });
};

const showSkillSettings = ref(false);
const selectedSkill = ref<any>(null);

const onSkillContextMenu = (p: { event: MouseEvent, skill: any }) => {
  selectedSkill.value = p.skill;
  showSkillSettings.value = true;
};

const handleGlobalKeyDown = (e: KeyboardEvent) => { 
  if (e.altKey && e.key.toLowerCase() === 'l') globalState.isLocked = !globalState.isLocked; 
  if (e.ctrlKey && e.key.toLowerCase() === 't') {
    e.preventDefault();
    if (globalState.isConnected) createNewTab();
  }
};

const isCtrlPressed = ref(false);

onMounted(async () => {
  await loadUIPreferences();
  // v2.14.2: Force unlock on fresh load to prevent interaction freeze
  globalState.isLocked = false;
  
  window.addEventListener('keydown', handleGlobalKeyDown);
  window.addEventListener('keydown', (e) => { if (e.ctrlKey) isCtrlPressed.value = true; });
  window.addEventListener('keyup', (e) => { if (!e.ctrlKey) isCtrlPressed.value = false; });
  document.addEventListener('mousemove', handleGlobalMouseMove);
  document.addEventListener('mouseup', () => { isResizingSFTP.value = false; document.body.style.cursor = ''; });
  window.addEventListener('close-all-menus', () => closeAllMenus());
  
  listen('conn-status', (e: any) => {
    storeActions.pushLog(e.payload);
  });

  listen('backend-log', (e: any) => {
    if (!isLogsPaused.value) {
      storeActions.pushLog(e.payload);
    }
  });
  listen('traffic-event', () => {
    isTrafficFlashing.value = true;
    if (trafficTimeout) clearTimeout(trafficTimeout);
    trafficTimeout = setTimeout(() => isTrafficFlashing.value = false, 150);
  });

  const saved = localStorage.getItem(storageKey(globalState.host));
  if (!globalState.isConnected && !saved) {
    await createNewTab("Main Shell", 'terminal', {}, false, "tab-1");
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeyDown);
  document.removeEventListener('mousemove', handleGlobalMouseMove);
  if (statsIntervalId) clearInterval(statsIntervalId);
});

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
       :class="{ 'safe-mode': globalState.isSafeMode }" 
       :style="{ 
         '--ter-ui-scale': uiPrefs.ui_scale,
         '--ter-glow-opacity': uiPrefs.glow_intensity / 100,
         '--ter-pulse-duration': (2.0 - (uiPrefs.pulse_speed / 100) * 1.8) + 's',
         'font-size': (14 * uiPrefs.ui_scale) + 'px'
       }"
       @click="closeAllMenus" @contextmenu="closeAllMenus">
    <CyberGate v-if="!globalState.isConnected" @connected="onConnected" />
    
    <div v-else class="main-view">
      <SettingsPanel :isOpen="globalState.showSettings" 
                     :useNativeWebview="useNativeWebview" 
                     :isSafeMode="globalState.isSafeMode"
                     :sidebarSlots="sidebarSlots"
                     :uiPrefs="uiPrefs"
                     @update:useNativeWebview="useNativeWebview = $event" 
                     @update:isSafeMode="storeActions.toggleSafeMode($event)"
                     @update:sidebarSlots="sidebarSlots = $event"
                     @update-layout="handleUpdateLayout"
                     @auto-detect="autoDetectScale"
                     @close="globalState.showSettings = false" @update-macros="(m) => activeMacros = m" />
      
      <main class="workspace" @click.stop>
        <!-- Modals and Context Menus -->
        <div v-if="showWebMenu" class="context-menu" :style="{ top: webMenuY + 'px', left: webMenuX + 'px' }">
          <header class="menu-header">WEB ACTIONS</header>
          <div class="menu-item" @click="invoke('reload_cyber_webview', { label: contextWebId! }); activeMenu = null">🔄 Reload</div>
          <div class="menu-item" @click="createWebview(); activeMenu = null">➕ New Web Instance</div>
          <div class="menu-divider"></div>
          <div class="menu-item danger" @click="closeWebview(contextWebId!); activeMenu = null">❌ Close Page</div>
        </div>

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

        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }">
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
        </div>

        <div v-if="showPrivilegeMenu" class="context-menu" :style="{ top: privilegeMenuY + 'px', left: privilegeMenuX + 'px' }">
          <header class="menu-header">CYBER PRIVILEGE: {{ privilegeModule.toUpperCase() }}</header>
          <div v-item">🛠️ Deep Diagnostic</div>
          <div class="menu-item">🛡️ Secure Isolation</div>
          <div class="menu-item highlight">☢️ Core Override</div>
        </div>

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

        <!-- Matrix Allocator Entry Point -->
        <div class="workspace-body">
          <MatrixAllocator :sharedProps="sharedProps" 
                           @switch-tab="bringToForeground"
                           @proc-context="onTerminalContextMenu"
                           @terminal-context="onTerminalContextMenu"
                           @run-skill="runSkill"
                           @fast-access="onFastAccess"
                           @explorer-context="onExplorerContextMenu"
                           @resize-sftp-start="handleResizeSFTP"
                           @view-changed="handleSidebarViewRevert"
                           @switch-web="activeWebviewId = $event"
                           @web-context="onWebContextMenu"
                           @skill-context="onSkillContextMenu"
                           @header-context="onHeaderContextMenu"
                           @open-trigger-settings="globalState.showSettings = true"
                           @cycle-health-mode="cycleHealthMode"
                           @resize-charts="resizeCharts"
                           @open-vault-entry="handleVaultEntry" />
        </div>

        <footer class="status-bar">
          <div class="status-left">
            <button class="status-btn sidebar-toggle" @click.stop="globalState.isSidebarOpen = !globalState.isSidebarOpen">
              {{ globalState.isSidebarOpen ? 'SIDE_HIDE' : 'SIDE_SHOW' }}
            </button>
            <span class="status-sep">|</span>
            <div class="status-item node-info" @click="globalState.showNetworkMatrix = true">
              NODE: {{ globalState.host }}
            </div>
            <span class="status-sep">|</span>
            <div class="status-item traffic-indicator" :class="{ 'flashing': isTrafficFlashing }">
              NET_TRAFFIC
            </div>
            <span class="status-sep">|</span>
            <div class="status-item agent-zone" 
                 :class="{ 'active': globalState.isConnected }"
                 @click="onAgentZoneClick"
                 @contextmenu.prevent>
              AGENT: {{ globalState.isConnected ? 'ACTIVE' : 'OFFLINE' }}
            </div>
          </div>

          <div class="hotkey-bar">
            <button class="status-btn modifier" @click="invoke('write_pty', { tabId: activeTabId, data: '\t' })">TAB</button>
            <button class="status-btn modifier" :class="{ 'active': isCtrlPressed }" @click="isCtrlPressed = !isCtrlPressed">CTRL</button>
            <button class="status-btn modifier" @click="invoke('write_pty', { tabId: activeTabId, data: '\x03' })">C-C</button>
            <button class="status-btn modifier" @click="invoke('write_pty', { tabId: activeTabId, data: '\x1b' })">ESC</button>
          </div>

          <div class="status-right">
            <button class="status-btn" :class="{ 'active': globalState.isSafeMode }" @click="storeActions.toggleSafeMode(!globalState.isSafeMode)">
              {{ globalState.isSafeMode ? 'SAFE_MODE: ON' : 'SAFE_MODE: OFF' }}
            </button>
            <span class="status-sep">|</span>
            <button class="status-btn" :class="{ 'clip-flash': isClipFlashing }" @click="copyLatestAI">CLIP</button>
            <span class="status-sep">|</span>
            <button class="status-btn" @click="captureAndUpload(false)">AUDIT_UI</button>
            <span class="status-sep">|</span>
            <button class="status-btn web-toggle" :class="{ 'active': globalState.cyberMode === 1 }" @click="globalState.cyberMode = globalState.cyberMode === 1 ? 0 : 1">
              WEB_ENGINE: {{ globalState.cyberMode === 1 ? 'ON' : 'OFF' }}
            </button>
            <span class="status-sep">|</span>
            <button class="status-btn auto-toggle" :class="{ 'active': isAutoPilot }" @click="isAutoPilot = !isAutoPilot">
              AUTO_SYNC: {{ isAutoPilot ? 'ON' : 'OFF' }}
            </button>
            <span class="status-sep">|</span>
            <button class="status-btn" @click="globalState.showSettings = true">SETTINGS</button>
            <span class="status-sep">|</span>
            <button class="status-btn lock-btn" :class="{ 'active': globalState.isLocked }" @click="globalState.isLocked = true">SYS_LOCK</button>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="globalState.isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="globalState.isLocked = false" />
    <NetworkMatrix v-if="globalState.showNetworkMatrix" :activeId="globalState.activeServerId" :activeTabId="activeTabId" @close="globalState.showNetworkMatrix = false" />
  </div>
</template>

<style scoped>
.app-shell { 
  height: 100vh; 
  width: 100vw; 
  background: #000; 
  color: #d4d4d8; 
  font-family: 'Inter', system-ui, -apple-system, sans-serif;
  overflow: hidden; 
  display: flex; 
  flex-direction: column; 
}

/* Monospace zones for technical clarity */
.app-shell :deep(.terminal-pane), 
.app-shell :deep(.logs-container), 
.app-shell :deep(.log-stream-static),
.app-shell :deep(.preview-text),
.app-shell :deep(.cyber-input),
.app-shell :deep(.address-bar-input),
.app-shell :deep(.branding-text),
.app-shell :deep(.status-btn),
.app-shell :deep(.status-item) {
  font-family: 'JetBrains Mono', 'Fira Code', 'Ubuntu Mono', monospace !important;
}

.main-view { display: flex; flex: 1; height: 100%; width: 100%; overflow: hidden; position: relative; }

:deep(.side-bar) {
  width: 100%;
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
  min-width: 0;
  width: 100%;
}
.workspace-body { flex: 1; display: flex; overflow: hidden; position: relative; width: 100%; }
.terminal-pane { flex: 1; height: 100%; min-width: 0; display: flex; flex-direction: column; overflow: hidden; background: #000; }
.cyber-pane { width: calc(420px * var(--ter-ui-scale)); height: 100%; border-left: 1px solid #27272a; display: none; flex-direction: column; background: #000; }
.cyber-pane.open { display: flex; }
.cyber-container { display: flex; flex-direction: column; height: 100%; }
.cyber-logs-view { flex: 0 0 30%; border-bottom: 1px solid #27272a; overflow: hidden; display: flex; flex-direction: column; }
.cyber-logs-view header { padding: calc(5px * var(--ter-ui-scale)) calc(10px * var(--ter-ui-scale)); font-size: calc(11px * var(--ter-ui-scale)); color: #71717a; border-bottom: 1px solid #18181b; letter-spacing: 0.5px; }
.logs-container { flex: 1; overflow-y: auto; padding: calc(10px * var(--ter-ui-scale)); font-size: calc(11px * var(--ter-ui-scale)); color: #a1a1aa; }
.cyber-webview-wrapper { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { padding: calc(5px * var(--ter-ui-scale)); background: #09090b; border-bottom: 1px solid #27272a; display: flex; gap: calc(5px * var(--ter-ui-scale)); }
.address-bar-input { flex: 1; background: #000; border: 1px solid #27272a; color: #22c55e; padding: 2px 8px; font-size: calc(11px * var(--ter-ui-scale)); outline: none; border-radius: 4px; }
.refresh-btn { background: #18181b; border: 1px solid #27272a; color: #22c55e; cursor: pointer; padding: 0 calc(8px * var(--ter-ui-scale)); border-radius: 4px; }

/* v2.11.43: Bookmarks Bar Styles */
.bookmarks-bar { display: flex; gap: calc(8px * var(--ter-ui-scale)); padding: calc(4px * var(--ter-ui-scale)) calc(8px * var(--ter-ui-scale)); background: #000; border-bottom: 1px solid #18181b; overflow-x: auto; scrollbar-width: none; }
.bookmarks-bar::-webkit-scrollbar { display: none; }
.bookmark-item { font-size: calc(9px * var(--ter-ui-scale)); color: #a1a1aa; padding: 2px 8px; border: 1px solid rgba(113, 113, 122, 0.5); border-radius: 4px; cursor: pointer; white-space: nowrap; transition: all 0.25s ease; background: rgba(24, 24, 27, 0.5); }
.bookmark-item:hover { color: #22c55e; border-color: #22c55e; background: rgba(34, 197, 94, 0.1); box-shadow: 0 0 calc(10px * var(--ter-ui-scale)) rgba(34, 197, 94, 0.3); transform: translateY(-1px); }

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
  height: calc(var(--ter-status-bar-height) * var(--ter-ui-scale)); 
  background: #09090b; 
  border-top: 1px solid #18181b; 
  display: flex; 
  justify-content: space-between; 
  align-items: center; 
  padding: 0 calc(12px * var(--ter-ui-scale)); 
  font-size: calc(11px * var(--ter-ui-scale)); 
  flex-shrink: 0; 
  z-index: 1000;
  text-transform: uppercase;
  letter-spacing: 1px;
}

.status-left, .status-right, .hotkey-bar { display: flex; align-items: center; gap: calc(8px * var(--ter-ui-scale)); }
.status-sep { color: #27272a; font-size: calc(10px * var(--ter-ui-scale)); margin: 0 calc(4px * var(--ter-ui-scale)); pointer-events: none; }

.traffic-indicator { font-size: calc(9px * var(--ter-ui-scale)); color: #52525b; transition: all 0.2s; font-family: 'JetBrains Mono', monospace; cursor: help; }
.traffic-indicator:hover { color: #71717a; }
.traffic-indicator.flashing { color: #22c55e; text-shadow: 0 0 calc(5px * var(--ter-ui-scale)) #22c55e; }

.status-btn { 
  background: transparent; 
  border: none; 
  color: #52525b; 
  cursor: pointer; 
  padding: calc(4px * var(--ter-ui-scale)) calc(8px * var(--ter-ui-scale)); 
  font-family: 'JetBrains Mono', monospace !important;
  font-size: calc(11px * var(--ter-ui-scale));
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  display: flex;
  align-items: center;
  border-radius: 4px;
}

.status-btn:hover { color: #fff; transform: scale(1.05); }
.status-btn.active { color: #fff; text-shadow: 0 0 calc(4px * var(--ter-ui-scale)) currentColor; animation: breathe 2s infinite ease-in-out; }

.agent-zone.active { color: #22c55e; text-shadow: 0 0 calc(5px * var(--ter-ui-scale)) rgba(34, 197, 94, 0.5); animation: breathe 2s infinite ease-in-out; }
.agent-zone.pressing { transform: scale(0.95); filter: brightness(1.5); }
.agent-zone { position: relative; cursor: pointer; padding: calc(4px * var(--ter-ui-scale)) calc(8px * var(--ter-ui-scale)); transition: all 0.1s; }

.morse-preview {
  position: absolute;
  top: calc(-18px * var(--ter-ui-scale));
  left: 50%;
  transform: translateX(-50%);
  background: #22c55e;
  color: #000;
  padding: calc(1px * var(--ter-ui-scale)) calc(4px * var(--ter-ui-scale));
  border-radius: 2px;
  font-size: calc(9px * var(--ter-ui-scale));
  font-weight: bold;
  opacity: 0;
  transition: opacity 0.2s;
  pointer-events: none;
}
.agent-zone.pressing .morse-preview, .agent-zone:hover .morse-preview { opacity: 1; }

.web-toggle.active { color: #3b82f6; text-shadow: 0 0 calc(5px * var(--ter-ui-scale)) rgba(59, 130, 246, 0.5); }
.auto-toggle.active { color: #a855f7; text-shadow: 0 0 calc(5px * var(--ter-ui-scale)) rgba(168, 85, 247, 0.5); }
.modifier.active { color: #a855f7; background: rgba(168, 85, 247, 0.1); border: 1px solid rgba(168, 85, 247, 0.2); }
.lock-btn:hover { color: #ef4444 !important; text-shadow: 0 0 calc(5px * var(--ter-ui-scale)) rgba(239, 68, 68, 0.5); }

@keyframes breathe {
  0%, 100% { opacity: 1; filter: brightness(1); }
  50% { opacity: 0.7; filter: brightness(1.3); }
}

.node-info { cursor: pointer; color: #71717a; transition: color 0.2s; font-size: calc(11px * var(--ter-ui-scale)); }
.node-info:hover { color: #a855f7; }

.context-menu { 
  position: fixed !important; 
  z-index: 99999 !important; 
  background: rgba(9, 9, 11, 0.95) !important; 
  backdrop-filter: blur(10px);
  border: 1px solid #22c55e !important; 
  padding: calc(10px * var(--ter-ui-scale)) !important; 
  box-shadow: 0 0 calc(10px * var(--ter-ui-scale)) #22c55e !important; 
  border-radius: 6px !important; 
}
.menu-header { padding: calc(4px * var(--ter-ui-scale)) calc(8px * var(--ter-ui-scale)); font-size: calc(10px * var(--ter-ui-scale)); color: #166534; border-bottom: 1px solid #18181b; margin-bottom: calc(4px * var(--ter-ui-scale)); letter-spacing: 0.5px; }
.menu-item { padding: calc(6px * var(--ter-ui-scale)) calc(12px * var(--ter-ui-scale)); font-size: calc(12px * var(--ter-ui-scale)); cursor: pointer; color: #d4d4d8; border-radius: 4px; margin-bottom: 1px; }
.menu-item:hover { background: #22c55e; color: #000; }
.menu-item.danger { color: #ef4444; }
.menu-item.danger:hover { background: #ef4444; color: #000; }
.menu-divider { height: 1px; background: #18181b; margin: calc(4px * var(--ter-ui-scale)) 0; }

.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.85); backdrop-filter: blur(5px); display: flex; align-items: center; justify-content: center; z-index: 20000; }
.cyber-card { background: #09090b; border: 1px solid #22c55e; padding: calc(30px * var(--ter-ui-scale)); min-width: calc(400px * var(--ter-ui-scale)); box-shadow: 0 0 calc(30px * var(--ter-ui-scale)) rgba(34, 197, 94, 0.2); border-radius: 8px; }
.cyber-title { color: #22c55e; font-size: calc(18px * var(--ter-ui-scale)); letter-spacing: 2px; margin-bottom: calc(15px * var(--ter-ui-scale)); }
.skill-form { display: flex; flex-direction: column; gap: calc(10px * var(--ter-ui-scale)); }
.label { font-size: calc(10px * var(--ter-ui-scale)); color: #71717a; text-transform: uppercase; }
.cyber-input { background: #000; border: 1px solid #27272a; color: #22c55e; padding: calc(8px * var(--ter-ui-scale)); font-size: calc(12px * var(--ter-ui-scale)); outline: none; width: 100%; border-radius: 4px; }
.btn-primary { background: #22c55e; color: #000; border: none; padding: calc(10px * var(--ter-ui-scale)); font-weight: bold; cursor: pointer; margin-top: calc(15px * var(--ter-ui-scale)); border-radius: 4px; font-size: calc(12px * var(--ter-ui-scale)); }

.preview-overlay { z-index: 30000; background: rgba(0, 0, 0, 0.9); backdrop-filter: blur(15px); }
.preview-card { width: 80vw; height: 80vh; max-width: 1000px; display: flex; flex-direction: column; padding: 0; overflow: hidden; border-color: #3b82f6; box-shadow: 0 0 calc(40px * var(--ter-ui-scale)) rgba(59, 130, 246, 0.2); }
.preview-header { display: flex; justify-content: space-between; align-items: center; padding: calc(15px * var(--ter-ui-scale)) calc(20px * var(--ter-ui-scale)); background: rgba(59, 130, 246, 0.1); border-bottom: 1px solid rgba(59, 130, 246, 0.2); }
.preview-header .title { font-size: calc(12px * var(--ter-ui-scale)); font-family: 'JetBrains Mono', monospace; color: #3b82f6; letter-spacing: 1px; }
.preview-header .close-btn { background: transparent; border: none; color: #71717a; cursor: pointer; font-size: calc(18px * var(--ter-ui-scale)); }
.preview-header .close-btn:hover { color: #fff; }
.preview-body { flex: 1; padding: calc(20px * var(--ter-ui-scale)); overflow: auto; background: #000; }
.preview-text { margin: 0; font-family: 'JetBrains Mono', monospace; font-size: calc(13px * var(--ter-ui-scale)); color: #d4d4d8; line-height: 1.6; white-space: pre-wrap; word-break: break-all; }

.app-shell :deep(.tab-item) { font-size: calc(12px * var(--ter-ui-scale)); letter-spacing: 0.5px; border-radius: 4px 4px 0 0; }
.app-shell :deep(.name), .app-shell :deep(.file-name) { font-size: calc(12px * var(--ter-ui-scale)); }
.app-shell :deep(.header-with-action) { font-size: calc(11px * var(--ter-ui-scale)); letter-spacing: 0.5px; }

.app-shell.safe-mode :deep(*) {
  text-shadow: none !important;
  box-shadow: none !important;
  animation: none !important;
  backdrop-filter: none !important;
  transition: none !important;
}
.app-shell.safe-mode :deep(.scanline) { display: none !important; }

.webview-container.grid-layout {
  display: grid !important;
  grid-template-columns: repeat(3, 1fr);
  grid-template-rows: repeat(2, 1fr);
  gap: 2px;
  background: #18181b !important;
}
.grid-slot { border: 1px solid #27272a; overflow: hidden; background: #000; }
</style>
