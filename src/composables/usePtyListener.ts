import { onMounted, onUnmounted, type Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';
import { globalState, backendLogs, activeTabId } from '../store';

/**
 * usePtyListener Composable
 * v2.17.0: FIXED Double-Echo (llss) and RPC Leak.
 * This version registers a data hook in TerminalManager instead of creating a second listener.
 */
export function usePtyListener(
  isAutoPilot: Ref<boolean>,
  lastAutoPilotTime: Ref<number>,
  activeTriggers: Ref<string[]>,
  captureAndUpload: (auto: boolean) => Promise<void>,
  refreshWebview: (url?: string) => Promise<void>,
  handleExtractDOM: () => Promise<void>,
  lastActivityMap: Ref<Record<string, number>>
) {

  const processPtyData = (id: string, text: string, _bytes: Uint8Array): boolean => {
    lastActivityMap.value[id] = Date.now();

    // v2.17.0: RPC Interception & Cleanup
    if (text.includes('[TER_RPC]')) {
      const rpcRegex = /\[TER_RPC\]\s*({.*?})/g;
      let match;
      let consumed = false;

      while ((match = rpcRegex.exec(text)) !== null) {
        if (!match[1]) continue;
        try {
          const rpc = JSON.parse(match[1]);
          consumed = true;

          if (rpc.action === 'screenshot') {
            captureAndUpload(true);
          } else if (rpc.action === 'navigate' && rpc.url) {
            refreshWebview(rpc.url);
          } else if (rpc.action === 'split_webview' && rpc.url) {
            window.dispatchEvent(new CustomEvent('ter-split-webview', { detail: { url: rpc.url } }));
          } else if (rpc.action === 'extract_dom') {
            handleExtractDOM();
          } else if (rpc.action === 'notify') {
            backendLogs.value.push(`[🔔 AI NOTIFY] ${rpc.msg || rpc.message}`);
          } else if (rpc.action === 'chart') {
            backendLogs.value.push(`[📊 AI CHART DATA] ${JSON.stringify(rpc.data)}`);
          }
        } catch (e) { console.warn("RPC Parse Error:", e); }
      }

      // Fix 4: Use a FRESH regex instance for replace() — the exec() loop above
      // has exhausted lastIndex, so reusing rpcRegex would match nothing and leak
      // raw RPC text into the terminal output.
      if (consumed && text.replace(/\[TER_RPC\]\s*({.*?})/g, '').trim() === '') {
        return true;
      }
    }

    // Status busy/connected logic
    if (globalState.connectionStatus === 'connected') {
      globalState.connectionStatus = 'busy';
      setTimeout(() => {
        if (globalState.connectionStatus === 'busy') globalState.connectionStatus = 'connected';
      }, 200);
    }

    // AutoPilot Logic
    if (isAutoPilot.value) {
      const pt = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, ''); // Strip ANSI
      const actionMatch = pt.match(/\[TER_ACTION:\s*(click|type)\((\d+)(?:,\s*"(.*?)")?\)\]/);

      if (actionMatch) {
        const action = actionMatch[1], eid = actionMatch[2], txt = actionMatch[3] || "";
        const code = action === 'click' ? `window.TerAgent.click(${eid})` : `window.TerAgent.type(${eid}, ${JSON.stringify(txt)})`;
        if (activeTabId.value) invoke('eval_cyber_webview', { label: activeTabId.value, code });
        return true; // Consume action trigger
      } else if (!pt.includes('tab-') && (Date.now() - lastAutoPilotTime.value) > 500) {
        const lm = pt.match(/http:\/\/localhost:(\d+)/);
        if (lm && lm[1]) {
          const port = parseInt(lm[1]);
          if (globalState.currentAgentPort !== port) {
            globalState.currentAgentPort = port;
            refreshWebview(`http://localhost:${port}`);
          }
        }

        if (activeTriggers.value.some(t => pt.includes(t))) {
          lastAutoPilotTime.value = Date.now();
          setTimeout(() => { invoke('write_pty', { tabId: id, data: "\r" }); }, 300);
        }
      }
    }

    return false; // Not consumed
  };

  onMounted(() => {
    // Register the processor in the singleton TerminalManager
    terminalManager.setDataHook(processPtyData);
  });

  onUnmounted(() => {
    // We don't necessarily want to null it if other components use it,
    // but in current architecture, App.vue manages usePtyListener lifecycle.
  });

  return { setupPtyListener: () => { } }; // Dummy for compatibility
}
