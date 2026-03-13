import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export function useCyber(activeTabId: any, backendLogs: any) {
  const previewUrl = ref('http://localhost:5173');
  const isWebviewLoading = ref(false);
  const useNativeWebview = ref(true);

  const refreshWebview = async (fUrl?: string) => {
    if (fUrl) previewUrl.value = fUrl;
    let u = previewUrl.value.trim();
    if (!u) return;

    // Support just typing the port
    if (/^\d+$/.test(u)) {
      u = `http://localhost:${u}`;
      previewUrl.value = u;
    }

    // Attempt to tunnel ANY port access that looks like localhost or is a remote port the user might want to access via SSH
    // If we are connected to SSH, we prefer tunneling to bypass Mixed Content blocks on Linux
    const m = u.match(/(?:localhost|127\.0\.0\.1|[\w\.-]+):(\d+)/); 
    if (m && m[1]) {
      const port = parseInt(m[1]);
      // Avoid tunneling the dev server port if we're in dev mode
      if (port === 5173 && (u.includes('localhost') || u.includes('127.0.0.1'))) {
         // Do nothing, just navigate
      } else {
        isWebviewLoading.value = true; 
        backendLogs.value.push(`[SYSTEM] Attempting SSH tunnel for port ${port}...`);
        try { 
          const p = await invoke<number>('open_dynamic_tunnel', { remotePort: port }); 
          if (p > 0) {
            previewUrl.value = `http://localhost:${p}`; 
            backendLogs.value.push(`[SYSTEM] Tunnel active: localhost:${p} -> remote:${port}`);
          } else {
            backendLogs.value.push(`[ERROR] Tunnel returned invalid port 0`);
          }
        } catch (e) {
          backendLogs.value.push(`[ERROR] Tunnel failed: ${e}`);
          console.error("Failed to open dynamic tunnel:", e);
        } finally { 
          isWebviewLoading.value = false; 
        } 
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
    if (!auto) backendLogs.value.push(`[SYSTEM] UI Snapshot triggered (stub)...`);
    // Removed call to ai_audit_ui as it is not implemented in backend
  };

  return {
    previewUrl,
    isWebviewLoading,
    useNativeWebview,
    refreshWebview,
    handleExtractDOM,
    onDomExtracted,
    captureAndUpload
  };
}
