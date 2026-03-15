import { ref, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';

export function useContextMenu(
  activeTabId: Ref<string | null>,
  renameTab: (id: string, name: string) => void,
  host: Ref<string>,
  currentPath: Ref<string>,
  currentAgentPort: Ref<number | null>,
  terminalTabs: Ref<any[]>
) {
  const showContextMenu = ref(false);
  const menuX = ref(0);
  const menuY = ref(0);
  const contextMenuTabId = ref<string | null>(null);
  const hasErrorSelection = ref(false);

  // ... (calculateMenuPosition, onTerminalContextMenu, copySelectedText, pasteFromClipboard, renameTabAction remains similar)

  const calculateMenuPosition = (e: MouseEvent, estimatedHeight = 350, estimatedWidth = 160) => {
    let x = e.clientX + 5, y = e.clientY + 5; // v2.13.3: Offset slightly
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

  const copyRuntimeEnv = () => {
    const env = `/// TER_RUNTIME_ENV\nHOST: ${host.value}\nCWD: ${currentPath.value}\nAGENT_PORT: ${currentAgentPort.value}\nTIMESTAMP: ${new Date().toISOString()}`;
    navigator.clipboard.writeText(env);
    showContextMenu.value = false;
  };

  const generateRunReport = async () => {
    if (!contextMenuTabId.value) return;
    const tid = contextMenuTabId.value;
    const tab = terminalTabs.value.find(t => t.id === tid);
    try {
      const logs = await invoke<number[][]>('get_terminal_logs', { tabId: tid, limit: 100 });
      const decoder = new TextDecoder();
      const text = logs.map(chunk => decoder.decode(new Uint8Array(chunk))).join('').replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
      const report = `/// TER_RUN_REPORT\nTAB: ${tab?.title || tid}\nHOST: ${host.value}\nDATE: ${new Date().toLocaleString()}\n\n--- RECENT_LOGS ---\n${text.substring(Math.max(0, text.length - 2000))}`;
      navigator.clipboard.writeText(report);
    } catch (e) {
      console.error("Report fail", e);
    }
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
    copyRuntimeEnv,
    generateRunReport,
    diagnoseSelection,
    calculateMenuPosition
  };
}
