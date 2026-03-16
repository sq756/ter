import { onMounted, onUnmounted, type Ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';
import { globalState, activeTabId, storeActions } from '../store';

// v2.11.56: Rendering Throttling
const writeQueues: Map<string, Uint8Array[]> = new Map();
let rafActive = false;

const processWriteQueues = () => {
  let hasMore = false;
  writeQueues.forEach((queue, id) => {
    if (queue.length > 0) {
      const totalLength = queue.reduce((acc, curr) => acc + curr.length, 0);
      const combined = new Uint8Array(totalLength);
      let offset = 0;
      for (const buf of queue) {
        combined.set(buf, offset);
        offset += buf.length;
      }
      terminalManager.write(id, combined);
      queue.length = 0;
      hasMore = true;
    }
  });
  
  if (hasMore) {
    requestAnimationFrame(processWriteQueues);
  } else {
    rafActive = false;
  }
};

/**
 * usePtyListener Composable
 * v2.14.0: Migrated to Global Store for dynamic tiling.
 */
export function usePtyListener(
  isAutoPilot: Ref<boolean>,
  lastAutoPilotTime: Ref<number>,
  activeTriggers: Ref<string[]>,
  captureAndUpload: (auto: boolean) => Promise<void>,
  refreshWebview: (url?: string) => Promise<void>,
  handleExtractDOM: () => Promise<void>,
  lastActivityMap: Ref<Record<string, number>>,
  explorerActionDownload?: (onStatus?: (s: string) => void, remotePathOverride?: string) => Promise<void>
) {
  let unlistenPty: any;
  const decoder = new TextDecoder('utf-8', { fatal: false });

  const setupPtyListener = async () => {
    unlistenPty = await listen<any>('pty-data', (ev) => {
      const { id, data } = ev.payload;
      if (!id || !data) return;

      lastActivityMap.value[id] = Date.now();

      let bytes = typeof data === 'string' ? new TextEncoder().encode(data) : new Uint8Array(data);
      let text = decoder.decode(bytes);

      if (text.includes('[TER_RPC]')) {
        const rpcRegex = /\[TER_RPC\]\s*(\{.*?\})/g;
        let match;
        let cleanedText = text;
        let foundRpc = false;

        while ((match = rpcRegex.exec(text)) !== null) {
          if (!match[1]) continue;
          try {
            const rpc = JSON.parse(match[1]);
            foundRpc = true;
            
            if (rpc.action === 'screenshot') {
              captureAndUpload(true);
            } else if (rpc.action === 'navigate' && rpc.url) {
              refreshWebview(rpc.url);
            } else if (rpc.action === 'extract_dom') {
              handleExtractDOM();
            } else if (rpc.action === 'notify') {
              storeActions.pushLog(`[🔔 AI NOTIFY] ${rpc.msg || rpc.message}`);
            } else if (rpc.action === 'chart') {
              storeActions.pushLog(`[📊 AI CHART DATA] ${JSON.stringify(rpc.data)}`);
            } else if (rpc.action === 'download' && rpc.path) {
              if (explorerActionDownload) {
                // v2.16.1: Direct download via RPC
                explorerActionDownload((msg) => storeActions.pushLog(`[STATUS] ${msg}`), rpc.path);
              }
            }
            
            cleanedText = cleanedText.replace(match[0], '');
          } catch (e) { console.warn("RPC Parse Error:", e); }
        }

        if (foundRpc) {
          if (cleanedText.trim() === '') return;
          bytes = new TextEncoder().encode(cleanedText);
        }
      }

      if (!writeQueues.has(id)) writeQueues.set(id, []);
      writeQueues.get(id)!.push(bytes);
      
      if (!rafActive) {
        rafActive = true;
        requestAnimationFrame(processWriteQueues);
      }
      
      if (globalState.connectionStatus === 'connected') { 
        globalState.connectionStatus = 'busy'; 
        setTimeout(() => { 
          if (globalState.connectionStatus === 'busy') globalState.connectionStatus = 'connected'; 
        }, 200); 
      }
      
      if (isAutoPilot.value) {
        const pt = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
        const actionMatch = pt.match(/\[TER_ACTION:\s*(click|type)\((\d+)(?:,\s*"(.*?)")?\)\]/);
        
        if (actionMatch) {
          const action = actionMatch[1], eid = actionMatch[2], txt = actionMatch[3] || "";
          const code = action === 'click' ? `window.TerAgent.click(${eid})` : `window.TerAgent.type(${eid}, ${JSON.stringify(txt)})`;
          if (activeTabId.value) invoke('eval_cyber_webview', { label: activeTabId.value, code });
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
    });
  };

  onMounted(() => {
    setupPtyListener();
  });

  onUnmounted(() => {
    if (unlistenPty) unlistenPty();
  });

  return { setupPtyListener };
}
