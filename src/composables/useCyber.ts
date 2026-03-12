import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export function useCyber(activeTabId: any, backendLogs: any) {
  const previewUrl = ref('http://localhost:5173');
  const isWebviewLoading = ref(false);

  const refreshWebview = async (fUrl?: string) => {
    if (fUrl) previewUrl.value = fUrl;
    let u = previewUrl.value.trim();
    if (!u) return;

    if (/^\d+$/.test(u)) {
      u = `http://localhost:${u}`;
      previewUrl.value = u;
    }

    const m = u.match(/(?:localhost|127\.0\.0\.1):(\d+)/); 
    if (m && m[1]) { 
      isWebviewLoading.value = true; 
      try { 
        const p = await invoke<number>('open_dynamic_tunnel', { remotePort: parseInt(m[1]) }); 
        previewUrl.value = `http://localhost:${p}`; 
      } catch (e) {
        console.error("Failed to open dynamic tunnel:", e);
      } finally { 
        isWebviewLoading.value = false; 
      } 
    }
  };

  const handleExtractDOM = async () => { 
    backendLogs.value.push(`[INFO] Extracting DOM...`); 
    try {
      await invoke('extract_cyber_dom'); 
    } catch (e) {
      backendLogs.value.push(`[ERROR] DOM Extract Fail: ${e}`);
    }
  };

  const onDomExtracted = async (md: string) => { 
    if (activeTabId.value) { 
      try {
        await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~${md}\x1b[201~\r` }); 
        backendLogs.value.push(`[INFO] Snapshot injected.`); 
      } catch (e) {
        backendLogs.value.push(`[ERROR] Snapshot Injection Fail: ${e}`);
      }
    } 
  };

  const captureAndUpload = async (auto = false) => {
    if (!auto) backendLogs.value.push(`[SYSTEM] Initiating UI sync...`);
    try {
      const path = await invoke<string>('ai_audit_ui');
      if (!auto) backendLogs.value.push(`[INFO] UI Snapshot saved: ${path}`);
    } catch (e) {
      backendLogs.value.push(`[ERROR] Audit Fail: ${e}`);
    }
  };

  return {
    previewUrl,
    isWebviewLoading,
    refreshWebview,
    handleExtractDOM,
    onDomExtracted,
    captureAndUpload
  };
}
