import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { globalState, activeTabId } from '../store';

export const sanitizeSftpPath = (p: string): string => {
  if (!p) return '/';
  if (p === '..') return p;
  if (p === '.') return p;
  
  if (!p.startsWith('/')) {
    const slashIdx = p.indexOf('/');
    if (slashIdx === -1) {
      return '/';
    } else {
      return p.substring(slashIdx);
    }
  }
  return p;
};

/**
 * useExplorer Composable
 * v2.14.0: Migrated to Global Store for dynamic tiling.
 */
export function useExplorer() {
  const realFiles = ref<any[]>([]);

  const refreshExplorer = async (pathOverride?: string) => {
    if (!globalState.isConnected) {
      console.warn("[Explorer] Skip refresh: Not connected");
      return;
    }

    let targetPath = pathOverride || globalState.currentPath;
    targetPath = sanitizeSftpPath(targetPath);

    console.log("[Explorer] Fetching files for path:", targetPath);

    try {
      const content = await invoke<any>('ls_remote', { path: targetPath });      
      realFiles.value = content.files || [];
      globalState.currentPath = content.current_path; 
      console.log("[Explorer] Current Path confirmed:", globalState.currentPath);
    } catch (e) {
      console.error("[Explorer] Failed to refresh explorer:", e);
      realFiles.value = [];
    }
  };

  const changeDir = (p: string) => {
    if (p.startsWith('/')) {
      refreshExplorer(p);
      return;
    }

    let target = globalState.currentPath;
    if (p === '..') {
      if (globalState.currentPath === '/') return; 
      const pts = globalState.currentPath.split('/').filter(x => x);
      pts.pop();
      target = '/' + pts.join('/');
    } else {
      target = (globalState.currentPath === '/' ? '' : globalState.currentPath) + '/' + p;
    }
    
    refreshExplorer(target);
    
    setTimeout(() => {
      try {
        const s = localStorage.getItem('ter_fast_access');
        let l = s ? JSON.parse(s) : []; 
        if (!Array.isArray(l)) l = [];
        l = [globalState.currentPath, ...l.filter((x: string) => x !== globalState.currentPath)].slice(0, 5); 
        localStorage.setItem('ter_fast_access', JSON.stringify(l)); 
      } catch (e) {}
    }, 500);
  };

  const onFastAccess = async (p: string) => { 
    if (activeTabId.value) {
      await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${p}"\r` }); 
    }
    setTimeout(() => refreshExplorer(p), 300); 
  };

  return {
    realFiles,
    refreshExplorer,
    changeDir,
    onFastAccess
  };
}
