import { ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export function useExplorer(isConnected: Ref<boolean>, activeTabId: Ref<string | null>) {
  const currentPath = ref('/');
  const realFiles = ref<any[]>([]);

  const refreshExplorer = async () => {
    if (isConnected.value) {
      console.log("[Explorer] Fetching files for path:", currentPath.value);
      try {
        const files = await invoke<any[]>('ls_remote', { path: currentPath.value });
        realFiles.value = files || [];
        console.log("[Explorer] Received files:", realFiles.value.length);
      } catch (e) {
        console.error("[Explorer] Failed to refresh explorer:", e);
        realFiles.value = [];
      }
    } else {
      console.warn("[Explorer] Skip refresh: Not connected");
    }
  };

  const changeDir = (p: string) => {
    if (p === '..') {
      const pts = currentPath.value.split('/').filter(x => x);
      pts.pop();
      currentPath.value = '/' + pts.join('/');
    } else {
      currentPath.value = (currentPath.value === '/' ? '' : currentPath.value) + '/' + p;
    }
    
    // Track fast access
    try {
      const s = localStorage.getItem('ter_fast_access');
      let l = s ? JSON.parse(s) : []; 
      if (!Array.isArray(l)) l = [];
      l = [currentPath.value, ...l.filter((x: string) => x !== currentPath.value)].slice(0, 5); 
      localStorage.setItem('ter_fast_access', JSON.stringify(l)); 
    } catch (e) {
      console.warn('Fast access track failed', e);
      localStorage.setItem('ter_fast_access', JSON.stringify([currentPath.value]));
    }
    
    refreshExplorer();
  };

  const onFastAccess = async (p: string) => { 
    currentPath.value = p; 
    if (activeTabId.value) {
      await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${p}"\r` }); 
    }
    refreshExplorer(); 
  };

  return {
    currentPath,
    realFiles,
    refreshExplorer,
    changeDir,
    onFastAccess
  };
}
