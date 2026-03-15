import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';
import { 
  terminalTabs, activeTabId, activeTabIdSecondary, splitMode, 
  globalState, backendLogs 
} from '../store';

export type ViewType = 'terminal' | 'webview' | 'editor';

/**
 * useTabs Composable
 * v2.14.0: Migrated to Global Store for dynamic tiling.
 */
export function useTabs() {
  const lastActivityMap = ref<Record<string, number>>({});
  const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

  const createNewTab = async (title = "Shell", viewType: ViewType = 'terminal', data: any = {}, skipPty = false, existingId?: string) => {
    const id = existingId || 'tab-' + Math.random().toString(36).substr(2, 9);
    console.log(`[useTabs] Creating tab: ${id} (${title}) type: ${viewType}`);
    
    if (viewType === 'terminal') {
      terminalManager.setOnDataCallback(id, (tid, data) => { 
        if (!skipPty && globalState.isConnected) {
          invoke('write_pty', { tabId: tid, data }).catch(e => console.error("Write fail:", e)); 
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
    lastActivityMap.value[id] = Date.now();
    return id;
  };

  const toggleSplit = () => {
    splitMode.value = !splitMode.value;
    if (splitMode.value && !activeTabIdSecondary.value) {
      const other = terminalTabs.value.find(t => !t.isBackground && t.id !== activeTabId.value);
      if (other) activeTabIdSecondary.value = other.id;
    }
  };

  const closeTab = (id: string) => {
    const idx = terminalTabs.value.findIndex(t => t.id === id);
    if (idx !== -1) {
      terminalTabs.value.splice(idx, 1);
      terminalManager.remove(id);
      if (activeTabId.value === id) {
        activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null;
      }
    }
  };

  const sendToBackground = (id: string | null) => {
    const tid = id || activeTabId.value;
    if (tid) {
      const tab = terminalTabs.value.find(t => t.id === tid);
      if (tab) {
        const s = terminalManager.getSelection(tab.id).trim();
        const currentTitle = tab.title;
        const isCustomName = currentTitle !== 'Shell' && currentTitle !== 'Main Shell' && !currentTitle.startsWith('Proc:') && !currentTitle.startsWith('tab-');
        if (!isCustomName) {
          tab.title = s ? `Proc: ${s.substring(0, 10)}...` : `Proc: ${tid.substring(0, 5)}`;
        }
        tab.isBackground = true;
        if (activeTabId.value === tid) {
          activeTabId.value = terminalTabs.value.find(t => !t.isBackground)?.id || null;
        }
      }
    }
  };

  const bringToForeground = (id: string) => {
    const t = terminalTabs.value.find(t => t.id === id);
    if (t) {
      t.isBackground = false;
      activeTabId.value = id;
    }
  };

  const renameTab = (id: string, newName: string) => {
    const t = terminalTabs.value.find(x => x.id === id);
    if (t) t.title = newName;
  };

  return {
    terminalTabs,
    activeTabId,
    activeTabIdSecondary,
    splitMode,
    toggleSplit,
    backgroundTabs,
    createNewTab,
    closeTab,
    sendToBackground,
    bringToForeground,
    renameTab,
    lastActivityMap
  };
}
