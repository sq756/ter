import { reactive, computed, shallowRef, ref, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from './TerminalManager';

/**
 * TER_CORE GLOBAL STATE CENTER
 * v2.14.1: Enhanced with actions for dynamic tiling.
 */

export const globalState = reactive({
  // Connection State
  isConnected: false,
  host: 'Remote Server',
  activeServerId: null as string | null,
  connectionStatus: 'disconnected' as 'connected' | 'busy' | 'disconnected',
  
  // UI State
  isLocked: false,
  isSafeMode: localStorage.getItem('ter_safe_mode') === 'true',
  useNativeWebview: localStorage.getItem('ter_use_native_webview') !== 'false', // Default true
  showSettings: false,
  showNetworkMatrix: false,
  isSidebarOpen: true,
  cyberMode: 0,
  sftpHeight: Number(localStorage.getItem('ter_sftp_height')) || 200,
  gridMode: localStorage.getItem('ter_grid_mode') === 'true',

  // Explorer State
  currentPath: '/',

  currentAgentPort: null as number | null,

  // Layout State (v2.14.7: Matrix Allocator)
  workspaceMatrix: (() => {
    try {
      const saved = JSON.parse(localStorage.getItem('ter_matrix') || 'null');
      if (saved && saved.version === 1) return saved;
      return null;
    } catch { return null; }
  })() || {
    version: 1,
    zoneLeft: 'SIDEBAR_PANEL',
    zoneMain: 'TERMINAL_MAIN',
    zoneRight: 'NONE',
    leftRatio: 25, // percentage
    rightRatio: 25 // percentage
  },

  // Cursor State (v2.14.14)
  cursorConfig: (() => {
    try {
      const saved = JSON.parse(localStorage.getItem('ter_cursor') || 'null');
      return saved || {
        enabled: true,
        size: 8,
        glow: 15,
        color: '#ffffff',
        breathing: true
      };
    } catch { return { enabled: true, size: 8, glow: 15, color: '#ffffff', breathing: true }; }
  })(),
});

export const backendLogs = shallowRef<string[]>([]);
export const terminalTabs = ref<any[]>([]);
export const activeTabId = ref<string | null>(null);
export const activeTabIdSecondary = ref<string | null>(null);
export const splitMode = ref(false);
export const webviewInstances = ref<any[]>([]);
export const activeWebviewId = ref<string | null>(null);

export const hostId = computed(() => globalState.isConnected ? globalState.host : 'GLOBAL');

// --- Log Ring Buffer & Throttling System ---
const LOG_MAX_LINES = 2000;
let logBuffer: string[] = [];
let logThrottleId: any = null;

const flushLogBuffer = () => {
  if (logBuffer.length > 0) {
    const current = backendLogs.value;
    let next = current.concat(logBuffer);
    if (next.length > LOG_MAX_LINES) {
      next = next.slice(next.length - LOG_MAX_LINES);
    }
    backendLogs.value = next;
    logBuffer = [];
  }
  logThrottleId = null;
};

export const storeActions = {
  pushLog(log: string) {
    logBuffer.push(log);
    if (!logThrottleId) {
      // Throttle DOM updates to approximately 60fps (16ms) or 50ms batching
      logThrottleId = setTimeout(flushLogBuffer, 50);
    }
  },

  setConnected(status: boolean, label?: string, id?: string) {
    globalState.isConnected = status;
    if (label) globalState.host = label;
    if (id) globalState.activeServerId = id;
    globalState.connectionStatus = status ? 'connected' : 'disconnected';
  },
  
  syncLogs(logs: string[]) {
    backendLogs.value = logs;
  },
  
  updatePath(path: string) {
    globalState.currentPath = path;
  },
  
  toggleSafeMode(val: boolean) {
    globalState.isSafeMode = val;
    localStorage.setItem('ter_safe_mode', val.toString());
  },

  // Tab Actions
  bringToForeground(id: string) {
    const t = terminalTabs.value.find(t => t.id === id);
    if (t) {
      t.isBackground = false;
      activeTabId.value = id;
    }
  },

  async createNewTab(title = "Shell", viewType: any = 'terminal', data: any = {}, skipPty = false, existingId?: string) {
    const id = existingId || 'tab-' + Math.random().toString(36).substr(2, 9);
    
    if (viewType === 'terminal') {
      terminalManager.setOnDataCallback(id, (tid, d) => { 
        if (!skipPty && globalState.isConnected) {
          invoke('write_pty', { tabId: tid, data: d }).catch(() => {}); 
        }
      });
      terminalManager.getOrCreate(id);
      if (!skipPty && globalState.isConnected) {
        try {
          if (globalState.host === 'LOCAL') {
            await invoke('spawn_local_pty', { tabId: id });
          } else {
            await invoke('spawn_new_pty', { tabId: id });
          }
          setTimeout(() => invoke('write_pty', { tabId: id, data: "\n\r" }), 500);
        } catch (e) { 
          backendLogs.value.push(`[ERROR] PTY Spawn fail for ${id}: ${e}`); 
        }
      }
    }

    const exists = terminalTabs.value.find(t => t.id === id);
    if (!exists) {
      terminalTabs.value.push({ id, title, viewType, data, isBackground: false });
    }
    
    if (splitMode.value && activeTabId.value) {
      activeTabIdSecondary.value = id;
    } else {
      activeTabId.value = id;
    }
    return id;
  },

  closeTab(id: string) {
    const idx = terminalTabs.value.findIndex(t => t.id === id);
    if (idx !== -1) {
      terminalTabs.value.splice(idx, 1);
      terminalManager.remove(id);
      if (activeTabId.value === id) {
        activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null;
      }
    }
  }
};
