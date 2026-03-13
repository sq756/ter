import { ref, computed, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';

export function useTabs(isConnected: Ref<boolean>, backendLogs: Ref<string[]>) {
  const terminalTabs = ref<any[]>([]);
  const activeTabId = ref<string | null>(null);
  const lastActivityMap = ref<Record<string, number>>({});
  const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

  const createNewTab = async (title = "Shell", skipPty = false, existingId?: string) => {
    const id = existingId || 'tab-' + Math.random().toString(36).substr(2, 9);
    console.log(`[useTabs] Creating tab: ${id} (${title})`);
    
    terminalManager.setOnDataCallback(id, (tid, data) => { 
      if (!skipPty && isConnected.value) {
        invoke('write_pty', { tabId: tid, data }).catch(e => console.error("Write fail:", e)); 
      }
    });

    terminalManager.getOrCreate(id);

    if (!skipPty && isConnected.value) {
      try {
        await invoke('spawn_new_pty', { tabId: id });
        // CRITICAL: 500ms delay to ensure PTY is ready
        setTimeout(() => invoke('write_pty', { tabId: id, data: "\n\r" }), 500);
      } catch (e) { 
        backendLogs.value.push(`[ERROR] PTY Spawn fail for ${id}: ${e}`); 
      }
    }

    const exists = terminalTabs.value.find(t => t.id === id);
    if (!exists) {
      terminalTabs.value.push({ id, title, isBackground: false });
    }
    
    activeTabId.value = id;
    lastActivityMap.value[id] = Date.now();
    return id;
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
        // RESTORED: Intelligent Naming logic
        const s = terminalManager.getSelection(tab.id).trim();
        tab.isBackground = true;
        tab.title = s ? `Proc: ${s.substring(0, 20)}...` : `Task: ${tab.id.substring(0, 5)}`;
        
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
    backgroundTabs,
    createNewTab,
    closeTab,
    sendToBackground,
    bringToForeground,
    renameTab,
    lastActivityMap
  };
}
