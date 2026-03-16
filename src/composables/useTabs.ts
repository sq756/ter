import { computed, ref, onMounted, onUnmounted } from 'vue';
import { 
  terminalTabs, activeTabId, activeTabIdSecondary, splitMode, 
  globalState, storeActions 
} from '../store';
import { terminalManager } from '../TerminalManager';

export type ViewType = 'terminal' | 'webview' | 'editor';

/**
 * useTabs Composable
 * v2.14.0: Migrated to Global Store for dynamic tiling.
 */
export function useTabs() {
  const lastActivityMap = ref<Record<string, number>>({});
  const backgroundTabs = computed(() => terminalTabs.value.filter(t => t.isBackground));

  const handleTabActivity = (e: any) => {
    const { id, timestamp } = e.detail;
    lastActivityMap.value[id] = timestamp;
  };

  onMounted(() => {
    window.addEventListener('ter-tab-activity', handleTabActivity);
  });

  onUnmounted(() => {
    window.removeEventListener('ter-tab-activity', handleTabActivity);
  });

  const createNewTab = storeActions.createNewTab;

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
