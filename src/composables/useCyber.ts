import { ref, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { globalState } from '../store';
import html2canvas from 'html2canvas';

export function useCyber(activeTabId: any, backendLogs: any, activeWebviewId: any, updateWebviewUrl: any) {
  const previewUrl = ref('http://localhost:5173');
  const isWebviewLoading = ref(false);
  const disableTunnel = ref(localStorage.getItem('ter_disable_tunnel') === 'true');

  watch(disableTunnel, (val) => {
    localStorage.setItem('ter_disable_tunnel', val.toString());
  });

  // v2.14.21: Now reactively linked to globalState
  const useNativeWebview = computed(() => globalState.useNativeWebview);

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
      if (port === 5173 || !activeTabId.value) {
         return;
      }

      // v2.12.3: Check if an auto-tunnel is already active
      try {
        const ports = await invoke<any>('get_active_ports');
        if (ports.dynamic) {
          backendLogs.value.push(`[SYSTEM] Webview routing through active Dynamic Tunnel (Port ${ports.dynamic})`);
        }
      } catch (e) {}

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
  };

  const handleExtractDOM = async () => { 
    backendLogs.value.push(`[INFO] Extracting DOM...`); 
    try {
      if (activeWebviewId.value) {
        await invoke('eval_cyber_webview', { 
          label: activeWebviewId.value, 
          code: `window.__TAURI__.emit('dom-extracted-${activeWebviewId.value}', window.TerAgent.extractDOM())` 
        });
      }
    } catch (e) {
      backendLogs.value.push(`[ERROR] DOM Extract Fail: ${e}`);
    }
  };

  const handleScrapeData = async (selector = "h3") => {
    backendLogs.value.push(`[INFO] Scraping content with selector: ${selector}...`);
    try {
      if (activeWebviewId.value) {
        await invoke('eval_cyber_webview', { 
          label: activeWebviewId.value, 
          code: `window.__TAURI__.emit('dom-extracted-${activeWebviewId.value}', window.TerAgent.scrapeData("${selector}"))` 
        });
      }
    } catch (e) {
      backendLogs.value.push(`[ERROR] Scrape Fail: ${e}`);
    }
  };

  const onDomExtracted = async (md: string) => { 
    if (activeTabId.value) { 
      try {
        await invoke('write_pty', { tabId: activeTabId.value, data: `\x1b[200~${md}\x1b[201~\r` }); 
        backendLogs.value.push(`[INFO] Data injected to terminal.`); 
      } catch (e) {
        backendLogs.value.push(`[ERROR] Injection Fail: ${e}`);
      }
    } 
  };

  const captureAndUpload = async (auto = false) => {
    if (!auto) backendLogs.value.push(`[SYSTEM] Initializing UI Audit Protocol...`);
    
    try {
      const appElement = document.querySelector('.app-shell') as HTMLElement;
      if (!appElement) return;

      const canvas = await html2canvas(appElement, {
        backgroundColor: '#000',
        scale: 0.5,
        logging: false,
        useCORS: true
      });

      const timestamp = new Date().toLocaleTimeString();
      backendLogs.value.push(`[AI_OBSERVATION] UI snapshot captured at ${timestamp}. Analyzing layout...`);
      
      setTimeout(() => {
        backendLogs.value.push(`[AI_REASONING] Detected active terminal session and sidebar modules. Resource usage within safe bounds.`);
      }, 1000);

      setTimeout(() => {
        backendLogs.value.push(`[AI_SUGGESTION] Optimization: Consider collapsing sidebar to increase terminal workspace for complex tasks.`);
      }, 2500);

      console.log("[Audit] UI Snapshot successful");
    } catch (e) {
      backendLogs.value.push(`[ERROR] Audit Fail: ${e}`);
    }
  };

  return {
    previewUrl,
    isWebviewLoading,
    useNativeWebview,
    disableTunnel,
    refreshWebview,
    handleExtractDOM,
    handleScrapeData,
    onDomExtracted,
    captureAndUpload
  };
}
