import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export function useCyber(activeTabId: any, backendLogs: any, activeWebviewId: any, updateWebviewUrl: any) {
  const previewUrl = ref('http://localhost:5173');
  const isWebviewLoading = ref(false);
  const disableTunnel = ref(localStorage.getItem('ter_disable_tunnel') === 'true');

  watch(disableTunnel, (val) => {
    localStorage.setItem('ter_disable_tunnel', val.toString());
  });

  // v2.11.12: Persistent Native Webview Toggle
  const savedMode = localStorage.getItem('ter_use_native_webview');
  const useNativeWebview = ref(savedMode === null ? true : savedMode === 'true');

  watch(useNativeWebview, (val) => {
    localStorage.setItem('ter_use_native_webview', val.toString());
  });

  const refreshWebview = async (fUrl?: string) => {
    if (fUrl) previewUrl.value = fUrl;
    let u = previewUrl.value.trim();
    if (!u) return;

    if (/^\d+$/.test(u)) {
      u = `http://localhost:${u}`;
      previewUrl.value = u;
    }

    // Update instance state (v2.11.43)
    if (activeWebviewId.value) {
      updateWebviewUrl(activeWebviewId.value, u);
    }

    const m = u.match(/(?:localhost|127\.0\.0\.1|[\w\.-]+):(\d+)/);
 
    if (m && m[1] && !disableTunnel.value) {
      const port = parseInt(m[1]);
      if (port === 5173 && (u.includes('localhost') || u.includes('127.0.0.1'))) {
         // Do nothing
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
  };

  return {
    previewUrl,
    isWebviewLoading,
    useNativeWebview,
    disableTunnel,
    refreshWebview,
    handleExtractDOM,
    onDomExtracted,
    captureAndUpload
  };
}
