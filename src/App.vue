<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Webview } from '@tauri-apps/api/webview';

const appWindow = getCurrentWindow();

// Components
import { terminalManager } from './TerminalManager';
import MatrixScreen from './components/MatrixScreen.vue';
import SidebarPanel from './components/SidebarPanel.vue';
import TerminalTabs from './components/TerminalTabs.vue';
import SettingsPanel from './components/SettingsPanel.vue';
import CyberGate from './components/CyberGate.vue';
import NetworkMatrix from './components/NetworkMatrix.vue';

// Composables
import { useMorse } from './composables/useMorse';
import { useTabs } from './composables/useTabs';
import { useStats } from './composables/useStats';
import { useExplorer } from './composables/useExplorer';
import { useExplorerContextMenu } from './composables/useExplorerContextMenu';
import { useCyber } from './composables/useCyber';
import { useContextMenu } from './composables/useContextMenu';
import { usePtyListener } from './composables/usePtyListener';

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
const showNetworkMatrix = ref(false);
const isSidebarOpen = ref(true);
const isCtrlPressed = ref(false);
const cyberMode = ref(0); 
const agentToken = ref('');
const currentAgentPort = ref<number | null>(null);
const backendLogs = ref<string[]>([]);
const isLogsPaused = ref(false);
const skills = ref<any[]>([]);

// v2.10.3: Resizable SFTP
const sftpHeight = ref(200);
const isResizingSFTP = ref(false);
const isAuditMode = ref(false);

const startResizingSFTP = (e: MouseEvent) => {
  isResizingSFTP.value = true;
  document.body.style.cursor = 'ns-resize';
};

const stopResizingSFTP = () => {
  isResizingSFTP.value = false;
  document.body.style.cursor = '';
};

const reloadSkills = () => {
  invoke('load_remote_skills').then((s: any) => {
    skills.value = s;
  }).catch(() => {});
};

const storageKey = (h: string) => `ter_tabs_${h.replace(/\s+/g, '_')}`;
let statsIntervalId: any = null;

// ==========================================
// --- DECOUPLED LOGIC ---
// ==========================================
const { 
  terminalTabs, activeTabId, backgroundTabs, lastActivityMap,
  createNewTab, closeTab, sendToBackground, bringToForeground, renameTab 
} = useTabs(isConnected, backendLogs);

const {
  currentPath, realFiles, refreshExplorer, changeDir, onFastAccess
} = useExplorer(isConnected, activeTabId);

const {
  showExplorerMenu, explorerMenuX, explorerMenuY, selectedFile,
  onExplorerContextMenu, explorerActionCd, explorerActionCat, explorerActionVim, explorerActionCopyPath, explorerActionRun,
  explorerActionDownload, explorerActionUpload
} = useExplorerContextMenu(activeTabId, currentPath, refreshExplorer);

const {
  previewUrl, isWebviewLoading, refreshWebview: legacyRefreshWebview, handleExtractDOM, onDomExtracted, captureAndUpload
} = useCyber(activeTabId, backendLogs);

const {
  showContextMenu, menuX, menuY, contextMenuTabId, hasErrorSelection,
  onTerminalContextMenu, copySelectedText, pasteFromClipboard, 
  renameTabAction, copyTabIdAction, copyRuntimeEnv, generateRunReport, diagnoseSelection, calculateMenuPosition
} = useContextMenu(activeTabId, renameTab, host, currentPath, currentAgentPort, terminalTabs);

const { 
  cpuChartRef, memChartRef, netChartRef, currentCpuUsage, 
  healthMode, currentNetSpeed, extraStats,
  initCharts, fetchStats, setHealthMode
} = useStats(currentAgentPort, agentToken);

const { 
  morseSequence, morseText, showMorseMacro, isMorsePressed, possibleLetters,
  handleMorseWheel, onMorseMacro
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
const closeAllMenus = () => {
  showContextMenu.value = false;
  showExplorerMenu.value = false;
  showMorseMacro.value = false;
};

// v3.0 Native Webview Logic
const webviewContainerRef = ref<HTMLElement | null>(null);
let nativeWebview: Webview | null = null;
let resizeObserver: ResizeObserver | null = null;

const updateNativeWebviewPosition = async () => {
  if (!nativeWebview || !webviewContainerRef.value || cyberMode.value !== 1) return;
  
  const rect = webviewContainerRef.value.getBoundingClientRect();
  await nativeWebview.setSize({
    type: 'Logical',
    width: rect.width,
    height: rect.height
  });
  await nativeWebview.setPosition({
    type: 'Logical',
    x: rect.left,
    y: rect.top
  });
};

const refreshWebview = async () => {
  if (cyberMode.value !== 1) return;
  if (nativeWebview && safePreviewUrl.value) {
    await nativeWebview.navigate(safePreviewUrl.value);
  } else {
    spawnNativeWebview(safePreviewUrl.value);
  }
};

const spawnNativeWebview = async (url: string) => {
  if (!url || url === 'about:blank') {
    if (nativeWebview) await nativeWebview.hide();
    return;
  }

  const rect = webviewContainerRef.value?.getBoundingClientRect() || { left: 0, top: 0, width: 800, height: 600 };

  if (!nativeWebview) {
    nativeWebview = new Webview(appWindow, 'ter_main_webview', {
      url,
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
    });
    
    nativeWebview.once('tauri://created', () => {
      console.log('TER_SYSTEM: Native Webview spawned.');
    });
  } else {
    await nativeWebview.navigate(url);
    await updateNativeWebviewPosition();
    await nativeWebview.show();
  }
};

const onGlobalMouseMove = (e: MouseEvent) => {
  if (isResizingSFTP.value) {
    let newHeight = window.innerHeight - e.clientY - 30;
    if (newHeight > 100 && newHeight < window.innerHeight * 0.8) {
      sftpHeight.value = newHeight;
      nextTick(() => updateNativeWebviewPosition());
    }
  }
};

const safePreviewUrl = computed(() => {
  if (!previewUrl.value) return '';
  if (previewUrl.value.includes('localhost:1420') || previewUrl.value.includes('localhost:5173')) {
    return 'about:blank';
  }
  return previewUrl.value;
});

// Intercept mode changes to show/hide native webview
watch(cyberMode, (newMode) => {
  if (newMode === 1) {
    if (safePreviewUrl.value) spawnNativeWebview(safePreviewUrl.value);
  } else {
    if (nativeWebview) nativeWebview.hide();
  }
});

watch(safePreviewUrl, (newUrl) => {
  if (cyberMode.value === 1) spawnNativeWebview(newUrl);
});

let morseTimer: any = null;
let isLeftDown = false;
let isRightDown = false;
let morseTimerGlobal: any = null;

const commitMorse = async () => {
  // Use a stable reference to morseMap
  const morseMap: Record<string, string> = {
    '.-': 'A', '-...': 'B', '-.-.': 'C', '-..': 'D', '.': 'E', '..-.': 'F', '--.': 'G', '....': 'H', '..': 'I', '.---': 'J', '-.-': 'K', '.-..': 'L', '--': 'M', '-.': 'N', '---': 'O', '.--.': 'P', '--.-': 'Q', '.-.': 'R', '...': 'S', '-': 'T', '..-': 'U', '...-': 'V', '.--': 'W', '-..-': 'X', '-.--': 'Y', '--..': 'Z', '-----': '0', '.----': '1', '..---': '2', '...--': '3', '....-': '4', '.....': '5', '-....': '6', '--...': '7', '---..': '8', '----.': '9'
  };
  const char = morseMap[morseSequence.value];
  if (char && activeTabId.value) {
    morseText.value += char;
    await invoke('write_pty', { tabId: activeTabId.value, data: char });
  }
  morseSequence.value = '';
  setTimeout(() => { if (!morseSequence.value) morseText.value = ''; }, 2000);
};

const handleMorseMouse = (e: MouseEvent) => {
  e.preventDefault();
  e.stopPropagation();
  
  if (e.type === 'mousedown') {
    isMorsePressed.value = true;
    if (e.button === 0) {
      isLeftDown = true;
      morseTimer = setTimeout(() => {
        if (isLeftDown && !isRightDown) {
          calculateMenuPosition(e, 200);
          showMorseMacro.value = true;
        }
      }, 500);
    }
    if (e.button === 2) isRightDown = true;
  } else if (e.type === 'mouseup' || e.type === 'mouseleave') {
    clearTimeout(morseTimer);
    if (e.type === 'mouseup') {
      if (isLeftDown && isRightDown) {
        if (activeTabId.value) invoke('write_pty', { tabId: activeTabId.value, data: ' ' });
      } else if (e.button === 0 && !showMorseMacro.value) {
        morseSequence.value += '.';
        if (morseTimerGlobal) clearTimeout(morseTimerGlobal);
        morseTimerGlobal = setTimeout(commitMorse, 800);
      } else if (e.button === 2) {
        morseSequence.value += '-';
        if (morseTimerGlobal) clearTimeout(morseTimerGlobal);
        morseTimerGlobal = setTimeout(commitMorse, 800);
      }
    }
    if (e.button === 0) isLeftDown = false;
    if (e.button === 2) isRightDown = false;
    isMorsePressed.value = false;
  }
};

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

const onConnected = async (hostLabel: string) => {
  host.value = hostLabel;
  isConnected.value = true;
  connectionStatus.value = 'connected';
  
  const saved = localStorage.getItem(storageKey(host.value));
  if (saved) {
    try {
      const ts = JSON.parse(saved); 
      if (Array.isArray(ts) && ts.length > 0) {
        terminalTabs.value = ts;
        for (const t of ts) {
          await createNewTab(t.title, false, t.id);
        }
        activeTabId.value = ts.find((t: any) => !t.isBackground)?.id || ts[0]?.id;
      } else {
        await createNewTab("Main Shell", false, "tab-1");
      }
    } catch (e) { await createNewTab("Main Shell", false, "tab-1"); }
  } else {
    await createNewTab("Main Shell", false, "tab-1");
  }

  setTimeout(() => {
    refreshExplorer();
    reloadSkills();
    nextTick(() => {
      initCharts();
      if (statsIntervalId) clearInterval(statsIntervalId);
      statsIntervalId = setInterval(fetchStats, 3000);
    });
  }, 1000);
};

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
  window.addEventListener('keydown', handleGlobalKeyDown);
  window.addEventListener('keyup', (e) => { if (!e.ctrlKey) isCtrlPressed.value = false; });
  window.addEventListener('mousemove', onGlobalMouseMove);
  window.addEventListener('mouseup', stopResizingSFTP);
  window.addEventListener('click', closeAllMenus);
  window.addEventListener('resize', updateNativeWebviewPosition);
  
  if (webviewContainerRef.value) {
    resizeObserver = new ResizeObserver(() => {
      if (cyberMode.value === 1) {
        updateNativeWebviewPosition();
      }
    });
    resizeObserver.observe(webviewContainerRef.value);
  }

  listen<string>('backend-log', (e) => { 
    if (!isLogsPaused.value) {
      backendLogs.value.push(e.payload); 
      if (backendLogs.value.length > 500) backendLogs.value.shift(); 
    }
  });
});

onUnmounted(() => {
  window.removeEventListener('click', closeAllMenus);
  window.removeEventListener('mousemove', onGlobalMouseMove);
  window.removeEventListener('mouseup', stopResizingSFTP);
  window.removeEventListener('resize', updateNativeWebviewPosition);
  if (resizeObserver) resizeObserver.disconnect();
});
</script>

<template>
  <div class="app-shell">
    <CyberGate v-if="!isConnected" @connected="onConnected" />
    
    <div v-else class="main-view">
      <SettingsPanel :isOpen="showSettings" @close="showSettings = false" @update-macros="(m) => activeMacros = m" />
      
      <SidebarPanel 
        :class="{ 'collapsed': !isSidebarOpen }"
        :files="realFiles" :currentPath="currentPath" :bgTabs="backgroundTabs" :skills="skills"
        :lastActivityMap="lastActivityMap"
        :cpuChartRef="cpuChartRef" :memChartRef="memChartRef" :netChartRef="netChartRef"
        :healthMode="healthMode" :currentNetSpeed="currentNetSpeed" :extraStats="extraStats"
        :isAutoPilot="isAutoPilot"
        :sftpHeight="sftpHeight"
        :is-audit-mode="isAuditMode"
        @update:isAutoPilot="isAutoPilot = $event"
        @switch-tab="bringToForeground" @switch-mode="(mode: number) => cyberMode = mode"
        @view-history="viewHistory" @proc-context="(p: any) => onTerminalContextMenu({e: p.event, id: p.tab.id})" @run-skill="runSkill"
        @change-dir="changeDir" @open-trigger-settings="showSettings = true" @fast-access="onFastAccess"
        @explorer-context="onExplorerContextMenu" @cycle-health-mode="setHealthMode"
        @skill-context="onSkillContextMenu"
        @resize-sftp-start="startResizingSFTP"
        @reload-skills="reloadSkills"
        @update-audit-mode="(val) => isAuditMode = val"
        @refresh-explorer="refreshExplorer"
      />

      <main class="workspace" @click.stop>
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
        <div v-if="showContextMenu" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }" @click.stop>
          <header class="menu-header">TERMINAL ACTIONS</header>
          <div v-if="hasErrorSelection" class="menu-item highlight" @click="diagnoseSelection">🤖 Diagnose Error</div>
          <div class="menu-item" @click="renameTabAction">✏️ Rename</div>
          <div class="menu-item" @click="sendToBackground(contextMenuTabId!)">🚀 Background</div>
          <div class="menu-divider"></div>
          <div class="menu-item" @click="copySelectedText">📋 Copy</div>
          <div class="menu-item" @click="pasteFromClipboard">📥 Paste</div>
          <div class="menu-divider"></div>
          <div class="menu-item danger" @click="closeTab(contextMenuTabId!)">❌ Close</div>
        </div>

        <!-- Explorer Menu -->
        <div v-if="showExplorerMenu" class="context-menu" :style="{ top: explorerMenuY + 'px', left: explorerMenuX + 'px' }" @click.stop>
          <header class="menu-header">FILE ACTIONS</header>
          <template v-if="selectedFile?.is_dir">
            <div class="menu-item" @click="explorerActionCd">📂 Open Folder</div>
            <div class="menu-item" @click="explorerActionUpload">📤 Upload</div>
          </template>
          <template v-else>
            <div class="menu-item" @click="explorerActionDownload">📥 Download</div>
            <div class="menu-item" @click="explorerActionCat">👁️ View</div>
          </template>
        </div>

        <!-- Morse Macros -->
        <div v-if="showMorseMacro" class="context-menu" :style="{ top: menuY + 'px', left: menuX + 'px' }" @click.stop>
          <header class="menu-header">QUICK MACROS</header>
          <div v-for="m in activeMacros" :key="m.name" class="menu-item" @click="runMacro(m.cmd)">⚡ {{ m.name }}</div>
        </div>

        <div class="workspace-body">
          <section class="terminal-pane">
            <TerminalTabs 
              :tabs="terminalTabs" :activeTabId="activeTabId" :connectionStatus="connectionStatus" 
              @switch-tab="bringToForeground" @close-tab="closeTab" @new-tab="createNewTab()" 
              @terminal-context="onTerminalContextMenu" 
            />
          </section>
          <section class="cyber-pane" :class="{ 'open': cyberMode === 1 }">
            <div class="cyber-container">
              <div class="cyber-logs-view">
                <header><span class="title">Cyber Logs</span></header>
                <div class="logs-container">
                  <div v-for="(log, i) in backendLogs" :key="i" class="log-line">{{ log }}</div>
                </div>
              </div>
              <div class="cyber-webview-wrapper">
                <nav class="webview-address-bar">
                  <input v-model="previewUrl" @keyup.enter="refreshWebview()" class="address-bar-input" />
                  <button @click="refreshWebview()" class="refresh-btn">⚡</button>
                </nav>
                <div ref="webviewContainerRef" class="webview-container" style="flex: 1; display: flex; flex-direction: column; height: 100%;">
                   <!-- Native Webview will be anchored here -->
                </div>
              </div>
            </div>
          </section>
        </div>

        <footer class="status-bar">
          <div class="status-left">
            <button class="status-btn sidebar-toggle" @click.stop="isSidebarOpen = !isSidebarOpen">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><line x1="9" y1="3" x2="9" y2="21"></line></svg>
            </button>
            <div class="status-item" @click="showNetworkMatrix = true" style="cursor: pointer;" title="Open Network Topology Matrix">
              <span class="node-dot purple"></span> NODE: {{ host }}
            </div>
            <div class="status-divider"></div>
            <div class="status-item stealth-zone" 
                 @mousedown.prevent="handleMorseMouse" 
                 @mouseup.prevent="handleMorseMouse" 
                 @mouseleave="handleMorseMouse"
                 @contextmenu.prevent>
              <div class="tiny-dot" :class="{ 'active': isMorsePressed }"></div> AGENT: ACTIVE
            </div>
          </div>

          <div class="hotkey-bar">
            <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\t' })">TAB</button>
            <button class="kb-pendant" :class="{ 'active': isCtrlPressed }" @click="isCtrlPressed = !isCtrlPressed">CTRL</button>
            <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\x03' })">C-C</button>
            <button class="kb-pendant" @click="invoke('write_pty', { tabId: activeTabId, data: '\x1b' })">ESC</button>
          </div>

          <div class="status-right">
            <button class="status-btn" @click="captureAndUpload(false)">📸 Audit</button>
            <button class="status-btn" @click="cyberMode = cyberMode === 1 ? 0 : 1">{{ cyberMode === 1 ? '🖥️' : '🌐' }} Web</button>
            <div class="status-toggle">
              <span>Auto</span>
              <label class="mini-switch"><input type="checkbox" v-model="isAutoPilot" /><span class="slider"></span></label>
            </div>
            <button class="status-btn lock-btn" @click="isLocked = true">🔒 LOCK</button>
          </div>
        </footer>
      </main>
    </div>
    <MatrixScreen :isLocked="isLocked" :logs="backendLogs" :cpuUsage="currentCpuUsage ?? 0" @unlock="isLocked = false" />
    <NetworkMatrix v-if="showNetworkMatrix" @close="showNetworkMatrix = false" />
  </div>
</template>

<style scoped>
.app-shell { 
  height: 100vh; 
  width: 100vw; 
  background: #000; 
  color: #d4d4d8; 
  font-family: 'Inter', 'Ubuntu', 'Noto Sans', 'Segoe UI', system-ui, sans-serif;
  overflow: hidden; 
  display: flex; 
  flex-direction: column; 
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
}

.app-shell :deep(*) {
  font-family: 'Inter', 'Ubuntu', 'Noto Sans', 'Segoe UI', system-ui, sans-serif;
}

/* Maintain monospace for specific technical elements */
.app-shell :deep(.terminal-pane *), 
.app-shell :deep(.logs-container *), 
.app-shell :deep(.kb-pendant),
.app-shell :deep(.branding-text),
.app-shell :deep(.cyber-input),
.app-shell :deep(.address-bar-input) {
  font-family: 'JetBrains Mono', 'Ubuntu Mono', 'Fira Code', monospace !important;
}

.main-view { display: flex; flex: 1; height: 100%; width: 100%; overflow: hidden; position: relative; }

/* FIX: Ensure side-bar completely vanishes when collapsed */
:deep(.side-bar) {
  width: 260px;
  flex-shrink: 0;
  transition: width 0.2s cubic-bezier(0.4, 0, 0.2, 1), padding 0.2s ease, opacity 0.2s ease;
  overflow: hidden;
}

.app-shell :deep(.module) {
  border-radius: 6px;
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
.cyber-pane { width: 420px; height: 100%; border-left: 1px solid #27272a; display: none; flex-direction: column; background: #000; }
.cyber-pane.open { display: flex; }
.cyber-container { display: flex; flex-direction: column; height: 100%; }
.cyber-logs-view { flex: 0 0 30%; border-bottom: 1px solid #27272a; overflow: hidden; display: flex; flex-direction: column; }
.cyber-logs-view header { padding: 5px 10px; font-size: 11px; color: #71717a; border-bottom: 1px solid #18181b; letter-spacing: 0.5px; }
.logs-container { flex: 1; overflow-y: auto; padding: 10px; font-size: 11px; color: #a1a1aa; }
.cyber-webview-wrapper { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { padding: 5px; background: #09090b; border-bottom: 1px solid #27272a; display: flex; gap: 5px; }
.address-bar-input { flex: 1; background: #000; border: 1px solid #27272a; color: #22c55e; padding: 2px 8px; font-size: 11px; outline: none; border-radius: 4px; }
.refresh-btn { background: #18181b; border: 1px solid #27272a; color: #22c55e; cursor: pointer; padding: 0 8px; border-radius: 4px; }

.status-bar { height: 32px; background: #09090b; border-top: 1px solid #18181b; display: flex; justify-content: space-between; align-items: center; padding: 0 12px; font-size: 11px; flex-shrink: 0; z-index: 1000; }
.status-left, .status-right { display: flex; align-items: center; gap: 15px; }
.status-divider { width: 1px; height: 14px; background: #27272a; }
.node-dot { width: 6px; height: 6px; border-radius: 50%; background: #a855f7; display: inline-block; margin-right: 4px; box-shadow: 0 0 5px #a855f7; }
.tiny-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; display: inline-block; margin-right: 4px; }
.tiny-dot.active { box-shadow: 0 0 8px #22c55e; }
.sidebar-toggle { color: #22c55e !important; cursor: pointer; display: flex; align-items: center; border-radius: 4px; }
.status-btn { background: transparent; border: 1px solid transparent; color: #52525b; cursor: pointer; padding: 2px 6px; font-family: inherit; border-radius: 4px; transition: all 0.2s; }
.status-btn:hover { color: #fff; border-color: #27272a; background: rgba(255,255,255,0.05); }
.lock-btn { border: 1px solid #27272a !important; }
.lock-btn:hover { color: #ef4444 !important; border-color: rgba(239, 68, 68, 0.3) !important; background: rgba(239, 68, 68, 0.1) !important; }

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

.stealth-zone:active .tiny-dot {
  background-color: #ffffff !important;
  box-shadow: 0 0 10px #ffffff !important;
}
</style>