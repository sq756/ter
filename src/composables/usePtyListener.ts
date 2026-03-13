import { onMounted, onUnmounted, type Ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { terminalManager } from '../TerminalManager';

export function usePtyListener(
  activeTabId: Ref<string | null>,
  connectionStatus: Ref<'connected' | 'busy' | 'disconnected'>,
  backendLogs: Ref<string[]>,
  isAutoPilot: Ref<boolean>,
  lastAutoPilotTime: Ref<number>,
  activeTriggers: Ref<string[]>,
  captureAndUpload: (auto: boolean) => Promise<void>,
  refreshWebview: (url?: string) => Promise<void>,
  handleExtractDOM: () => Promise<void>,
  currentAgentPort: Ref<number | null>
) {
  let unlistenPty: any;
  const decoder = new TextDecoder('utf-8', { fatal: false });

  const setupPtyListener = async () => {
    unlistenPty = await listen<any>('pty-data', (ev) => {
      const { id, data } = ev.payload;
      if (!id || !data) return;

      let bytes = typeof data === 'string' ? new TextEncoder().encode(data) : new Uint8Array(data);
      let text = decoder.decode(bytes);

      // ==========================================
      // --- PTY RPC INTERCEPTOR ---
      // ==========================================
      if (text.includes('[TER_RPC]')) {
        const rpcRegex = /\[TER_RPC\]\s*({.*?})/g;
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
              backendLogs.value.push(`[🔔 AI NOTIFY] ${rpc.msg || rpc.message}`);
            } else if (rpc.action === 'chart') {
              backendLogs.value.push(`[📊 AI CHART DATA] ${JSON.stringify(rpc.data)}`);
            }
            
            cleanedText = cleanedText.replace(match[0], '');
          } catch (e) { console.warn("RPC Parse Error:", e); }
        }

        if (foundRpc) {
          if (cleanedText.trim() === '') return;
          bytes = new TextEncoder().encode(cleanedText);
        }
      }

      // Write to xterm via manager
      terminalManager.write(id, bytes);
      
      // Update status pulse
      if (connectionStatus.value === 'connected') { 
        connectionStatus.value = 'busy'; 
        setTimeout(() => { 
          if (connectionStatus.value === 'busy') connectionStatus.value = 'connected'; 
        }, 200); 
      }
      
      // AutoPilot Logic
      if (isAutoPilot.value) {
        const pt = text.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
        const actionMatch = pt.match(/\[TER_ACTION:\s*(click|type)\((\d+)(?:,\s*"(.*?)")?\)\]/);
        
        if (actionMatch) {
          const action = actionMatch[1], eid = actionMatch[2], txt = actionMatch[3] || "";
          const code = action === 'click' ? `window.TerAgent.click(${eid})` : `window.TerAgent.type(${eid}, ${JSON.stringify(txt)})`;
          invoke('eval_cyber_webview', { code });
        } else if (!pt.includes('tab-') && (Date.now() - lastAutoPilotTime.value) > 500) {
          const lm = pt.match(/http:\/\/localhost:(\d+)/); 
          if (lm && lm[1]) {
            const port = parseInt(lm[1]);
            currentAgentPort.value = port;
            refreshWebview(`http://localhost:${port}`);
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
