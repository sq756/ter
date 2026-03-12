import { ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';

export function useContextMenu(
  activeTabId: Ref<string | null>,
  renameTab: (id: string, name: string) => void
) {
  const showContextMenu = ref(false);
  const menuX = ref(0);
  const menuY = ref(0);
  const contextMenuTabId = ref<string | null>(null);
  const hasErrorSelection = ref(false);

  const calculateMenuPosition = (e: MouseEvent, estimatedHeight = 250, estimatedWidth = 160) => {
    let x = e.clientX, y = e.clientY;
    if (y + estimatedHeight > window.innerHeight) y = window.innerHeight - estimatedHeight - 10;
    if (x + estimatedWidth > window.innerWidth) x = window.innerWidth - estimatedWidth - 10;
    menuX.value = x; menuY.value = y;
  };

  const onTerminalContextMenu = (p: { e: MouseEvent, id: string }) => { 
    contextMenuTabId.value = p.id; 
    calculateMenuPosition(p.e); 
    const s = terminalManager.getSelection(p.id); 
    hasErrorSelection.value = s.toLowerCase().includes('error') || s.toLowerCase().includes('exception') || s.includes('\x1b[31m'); 
    showContextMenu.value = true; 
  };

  const copySelectedText = async () => { 
    const id = contextMenuTabId.value || activeTabId.value; 
    if (id) { 
      const s = terminalManager.getSelection(id); 
      if (s) await navigator.clipboard.writeText(s); 
    } 
    showContextMenu.value = false; 
  };

  const pasteFromClipboard = async () => { 
    const id = contextMenuTabId.value || activeTabId.value; 
    if (id) { 
      try { 
        const t = await navigator.clipboard.readText(); 
        if (t) invoke('write_pty', { tabId: id, data: t }); 
      } catch(e){} 
    } 
    showContextMenu.value = false; 
  };

  const renameTabAction = () => { 
    const id = contextMenuTabId.value; 
    if (id) { 
      const n = prompt("New name:"); 
      if (n) renameTab(id, n); 
    } 
    showContextMenu.value = false; 
  };

  const copyTabIdAction = async () => { 
    if (contextMenuTabId.value) await navigator.clipboard.writeText(contextMenuTabId.value); 
    showContextMenu.value = false; 
  };

  const diagnoseSelection = async () => { 
    const id = contextMenuTabId.value || activeTabId.value; 
    if (id) { 
      const s = terminalManager.getSelection(id); 
      if (activeTabId.value) {
        await invoke('write_pty', { 
          tabId: activeTabId.value, 
          data: `\x1b[200~帮我诊断并给方案：\n\n\`\`\`\n${s}\n\`\`\`\x1b[201~\r` 
        }); 
      }
    } 
    showContextMenu.value = false; 
  };

  return {
    showContextMenu,
    menuX,
    menuY,
    contextMenuTabId,
    hasErrorSelection,
    onTerminalContextMenu,
    copySelectedText,
    pasteFromClipboard,
    renameTabAction,
    copyTabIdAction,
    diagnoseSelection,
    calculateMenuPosition
  };
}
