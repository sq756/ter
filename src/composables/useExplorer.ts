import { ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export function useExplorer(isConnected: Ref<boolean>, activeTabId: Ref<string | null>) {
  const currentPath = ref('/');
  const realFiles = ref<any[]>([]);

  const refreshExplorer = async (pathOverride?: string) => {
    if (!isConnected.value) {
      console.warn("[Explorer] Skip refresh: Not connected");
      return;
    }
    
    const targetPath = pathOverride || currentPath.value;
    console.log("[Explorer] Fetching files for path:", targetPath);
    
    try {
      const content = await invoke<any>('ls_remote', { path: targetPath });
      realFiles.value = content.files || [];
      currentPath.value = content.current_path; // v2.11.52: Absolute sync
      console.log("[Explorer] Current Path confirmed:", currentPath.value);
    } catch (e) {
      console.error("[Explorer] Failed to refresh explorer:", e);
      realFiles.value = [];
    }
  };

  const changeDir = (p: string) => {
    // If it's a specific full path (from breadcrumbs or fast access)
    if (p.startsWith('/')) {
      refreshExplorer(p);
      return;
    }

    // Logic for relative movement
    let target = currentPath.value;
    if (p === '..') {
      if (currentPath.value === '/') return; // Boundary protection
      const pts = currentPath.value.split('/').filter(x => x);
      pts.pop();
      target = '/' + pts.join('/');
    } else {
      target = (currentPath.value === '/' ? '' : currentPath.value) + '/' + p;
    }
    
    refreshExplorer(target);
    
    // Track fast access (keep currentPath.value which is updated after refresh)
    setTimeout(() => {
      try {
        const s = localStorage.getItem('ter_fast_access');
        let l = s ? JSON.parse(s) : []; 
        if (!Array.isArray(l)) l = [];
        l = [currentPath.value, ...l.filter((x: string) => x !== currentPath.value)].slice(0, 5); 
        localStorage.setItem('ter_fast_access', JSON.stringify(l)); 
      } catch (e) {}
    }, 500);
  };

  const onFastAccess = async (p: string) => { 
    if (activeTabId.value) {
      await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${p}"\r` }); 
    }
    // v2.11.52: Wait for shell to actually change dir
    setTimeout(() => refreshExplorer(p), 300); 
  };

  return {
    currentPath,
    realFiles,
    refreshExplorer,
    changeDir,
    onFastAccess
  };
}
