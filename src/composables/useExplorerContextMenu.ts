import { ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export function useExplorerContextMenu(
  activeTabId: Ref<string | null>,
  currentPath: Ref<string>
) {
  const showExplorerMenu = ref(false);
  const explorerMenuX = ref(0);
  const explorerMenuY = ref(0);
  const selectedFile = ref<any>(null);

  const calculateExplorerMenuPosition = (e: MouseEvent, estimatedHeight = 150, estimatedWidth = 160) => {
    let x = e.clientX, y = e.clientY;
    if (y + estimatedHeight > window.innerHeight) y = window.innerHeight - estimatedHeight - 10;
    if (x + estimatedWidth > window.innerWidth) x = window.innerWidth - estimatedWidth - 10;
    explorerMenuX.value = x; explorerMenuY.value = y;
  };

  const onExplorerContextMenu = (p: { e: MouseEvent, file: any }) => {
    selectedFile.value = p.file;
    calculateExplorerMenuPosition(p.e);
    showExplorerMenu.value = true;
  };

  const getFullPath = () => {
    if (!selectedFile.value) return currentPath.value;
    if (selectedFile.value.name === '..') {
      const pts = currentPath.value.split('/').filter(x => x);
      pts.pop();
      const p = '/' + pts.join('/');
      return p === '//' ? '/' : p;
    }
    const p = (currentPath.value === '/' ? '' : currentPath.value) + '/' + selectedFile.value.name;
    return p;
  };

  const explorerActionCd = async () => {
    if (activeTabId.value) {
      let path = getFullPath();
      if (selectedFile.value && !selectedFile.value.is_dir) {
        path = currentPath.value; // cd to parent folder if it's a file
      }
      await invoke('write_pty', { tabId: activeTabId.value, data: `cd "${path}"\r` });
    }
    showExplorerMenu.value = false;
  };

  const explorerActionCat = async () => {
    if (activeTabId.value && selectedFile.value && !selectedFile.value.is_dir) {
      await invoke('write_pty', { tabId: activeTabId.value, data: `cat "${getFullPath()}"\r` });
    }
    showExplorerMenu.value = false;
  };

  const explorerActionVim = async () => {
    if (activeTabId.value && selectedFile.value && !selectedFile.value.is_dir) {
      await invoke('write_pty', { tabId: activeTabId.value, data: `vim "${getFullPath()}"\r` });
    }
    showExplorerMenu.value = false;
  };

  const explorerActionCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(getFullPath());
    } catch(e) {}
    showExplorerMenu.value = false;
  };
  
  const explorerActionRun = async () => {
    if (activeTabId.value && selectedFile.value && !selectedFile.value.is_dir) {
      await invoke('write_pty', { tabId: activeTabId.value, data: `"${getFullPath()}"\r` });
    }
    showExplorerMenu.value = false;
  };

  return {
    showExplorerMenu,
    explorerMenuX,
    explorerMenuY,
    selectedFile,
    onExplorerContextMenu,
    explorerActionCd,
    explorerActionCat,
    explorerActionVim,
    explorerActionCopyPath,
    explorerActionRun
  };
}