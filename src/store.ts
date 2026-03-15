import { reactive, computed, shallowRef, ref } from 'vue';

/**
 * TER_CORE GLOBAL STATE CENTER
 * v2.14.0: Initial implementation for data-driven tiling.
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
  showSettings: false,
  showNetworkMatrix: false,
  isSidebarOpen: true,
  cyberMode: 0,
  sftpHeight: Number(localStorage.getItem('ter_sftp_height')) || 200,
  gridMode: localStorage.getItem('ter_grid_mode') === 'true',
  
  // Explorer State
  currentPath: '/',
  
  // Security/Agent
  agentToken: '',
  currentAgentPort: null as number | null,
});

// Heavy data streams use shallowRef or ref outside reactive for performance
export const backendLogs = shallowRef<string[]>([]);
export const terminalTabs = ref<any[]>([]);
export const activeTabId = ref<string | null>(null);
export const activeTabIdSecondary = ref<string | null>(null);
export const splitMode = ref(false);
export const webviewInstances = ref<any[]>([]);
export const activeWebviewId = ref<string | null>(null);

// Shared computed properties
export const hostId = computed(() => globalState.isConnected ? globalState.host : 'GLOBAL');

// Basic actions to maintain state integrity
export const storeActions = {
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
  }
};
