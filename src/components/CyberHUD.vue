<script setup lang="ts">
import { computed, ref } from 'vue';
import { globalState, backendLogs, webviewInstances, activeWebviewId, activeTabId, storeActions } from '../store';
import { useCyber } from '../composables/useCyber';
import CyberWebview from './CyberWebview.vue';

const props = defineProps<{
  zoneId: string;
}>();

const tacticalLogs = computed(() => backendLogs.value.slice(-50));

const getLogColor = (log: string) => {
  if (log.includes('[ERROR]')) return '#ef4444';
  if (log.includes('[SYSTEM]') || log.includes('[STATUS]')) return '#22c55e';
  if (log.includes('[DEBUG]') || log.includes('[INFO]')) return '#888888';
  if (log.includes('AI') || log.includes('Reasoning')) return '#a855f7';
  return '#a1a1aa';
};

// v2.15.3: Instance Ownership
// Each HUD zone now owns a specific webview instance to prevent mirroring
const ownedInstanceId = computed(() => `web-${props.zoneId.toLowerCase()}`);

const currentInstance = computed(() => {
  const inst = webviewInstances.value.find(w => w.id === ownedInstanceId.value);
  if (inst) return inst;
  // Fallback: Create it if it doesn't exist (v2.15.3 Auto-provisioning)
  return null; 
});

const updateWebviewUrl = (id: string, url: string) => {
  const instance = webviewInstances.value.find(w => w.id === id);
  if (instance) instance.url = url;
};

const {
  previewUrl, isWebviewLoading, refreshWebview, handleScrapeData, onDomExtracted, disableTunnel, useNativeWebview
} = useCyber(activeTabId, backendLogs, ownedInstanceId, updateWebviewUrl);

// Sync local previewUrl with instance url when instance changes
watch(() => currentInstance.value?.url, (newUrl) => {
  if (newUrl && newUrl !== previewUrl.value) {
    previewUrl.value = newUrl;
  }
}, { immediate: true });

onMounted(() => {
  // Ensure the instance exists in the global store
  if (!webviewInstances.value.find(w => w.id === ownedInstanceId.value)) {
    webviewInstances.value.push({
      id: ownedInstanceId.value,
      title: `Web Deck [${props.zoneId}]`,
      url: 'http://localhost:5173',
      isActive: true
    });
  }
});

const getSlotStyle = (idx: number) => {
  if (!globalState.gridMode) return {};
  const row = Math.floor(idx / 3) + 1;
  const col = (idx % 3) + 1;
  return {
    gridRow: row,
    gridColumn: col,
    width: '100%',
    height: '100%',
    position: 'relative'
  };
};
</script>

<template>
  <div class="cyber-hud-container">
    <div class="cyber-logs-view">
      <header><span class="title">Cyber Logs (HUD)</span></header>
      <div class="logs-container">
        <div v-for="(log, i) in tacticalLogs" :key="i" class="log-line" :style="{ color: getLogColor(log) }">
          {{ log }}
        </div>
      </div>
    </div>
    <div class="cyber-webview-wrapper">
      <nav class="webview-address-bar">
        <div class="engine-indicator" :class="{ 'native': useNativeWebview }">
          {{ useNativeWebview ? '⚡ Native' : '🐢 Iframe' }}
        </div>
        <input v-model="previewUrl" @keyup.enter="refreshWebview(previewUrl)" class="address-bar-input" />
        <button @click="refreshWebview(previewUrl)" class="refresh-btn">⚡</button>
        <button @click="handleScrapeData()" class="refresh-btn" title="Scrape Page Content (h3)">📊</button>
        <button @click="globalState.gridMode = !globalState.gridMode" class="refresh-btn" :title="globalState.gridMode ? 'Exit Grid Mode' : 'Enter Grid Mode (3x2)'" :style="{ color: globalState.gridMode ? '#3b82f6' : '#71717a' }">
          <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>
        </button>
        <button @click="disableTunnel = !disableTunnel" class="refresh-btn" :title="disableTunnel ? 'Enable Remote Tunnel' : 'Disable Remote Tunnel'" :style="{ color: disableTunnel ? '#ef4444' : '#22c55e' }">
          <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 11V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v5"></path><rect x="5" y="11" width="14" height="10" rx="2"></rect><circle cx="12" cy="16" r="1"></circle></svg>
        </button>
      </nav>

      <div class="webview-container" 
           style="flex: 1; display: flex; flex-direction: column; height: 100%; background: #000;">
         <template v-if="useNativeWebview && !globalState.isSafeMode">
           <div v-if="currentInstance" class="grid-slot" style="width: 100%; height: 100%; position: relative;">
             <CyberWebview
               :id="currentInstance.id"
               :url="currentInstance.url"
               :isActive="activeTabId === 'HUD' || true"
               :isSafeMode="globalState.isSafeMode"
               :zoneId="zoneId"
               @dom-extracted="onDomExtracted"
             />
           </div>
         </template>
         <div v-else-if="globalState.isSafeMode" class="safe-mode-placeholder">
           <span class="icon">🛡️</span>
           <div class="msg">WEB_ENGINE_DISABLED_IN_SAFE_MODE</div>
           <button class="os-browser-btn" @click="storeActions.toggleSafeMode(false)">DISABLE_SAFE_MODE</button>
         </div>
         <iframe v-else :src="previewUrl" class="cyber-iframe" frameborder="0" style="flex: 1; width: 100%; height: 100%; background: #ffffff; border: none;"></iframe>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cyber-hud-container { display: flex; flex-direction: column; height: 100%; width: 100%; background: #000; }
.cyber-logs-view { flex: 0 0 30%; border-bottom: 1px solid #27272a; overflow: hidden; display: flex; flex-direction: column; }
.cyber-logs-view header { padding: calc(5px * var(--ter-ui-scale)) calc(10px * var(--ter-ui-scale)); font-size: calc(11px * var(--ter-ui-scale)); color: #71717a; border-bottom: 1px solid #18181b; letter-spacing: 0.5px; }
.logs-container { flex: 1; overflow-y: auto; padding: calc(10px * var(--ter-ui-scale)); font-size: calc(11px * var(--ter-ui-scale)); color: #a1a1aa; font-family: 'JetBrains Mono', monospace; }
.cyber-webview-wrapper { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
.webview-address-bar { padding: calc(5px * var(--ter-ui-scale)); background: #09090b; border-bottom: 1px solid #27272a; display: flex; gap: calc(5px * var(--ter-ui-scale)); }
.address-bar-input { flex: 1; background: #000; border: 1px solid #27272a; color: #22c55e; padding: 2px 8px; font-size: calc(11px * var(--ter-ui-scale)); outline: none; border-radius: 4px; font-family: 'JetBrains Mono', monospace; }
.refresh-btn { background: #18181b; border: 1px solid #27272a; color: #22c55e; cursor: pointer; padding: 0 calc(8px * var(--ter-ui-scale)); border-radius: 4px; }

.webview-container.grid-layout {
  display: grid !important;
  grid-template-columns: repeat(3, 1fr);
  grid-template-rows: repeat(2, 1fr);
  gap: 2px;
  background: #18181b !important;
}
.grid-slot { border: 1px solid #27272a; overflow: hidden; background: #000; }

.safe-mode-placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; background: #09090b; color: #71717a; gap: calc(15px * var(--ter-ui-scale)); font-family: 'JetBrains Mono', monospace; }
.safe-mode-placeholder .icon { font-size: calc(32px * var(--ter-ui-scale)); }
.safe-mode-placeholder .msg { font-size: calc(12px * var(--ter-ui-scale)); letter-spacing: 1px; }
.safe-mode-placeholder .os-browser-btn { background: #18181b; border: 1px solid #27272a; color: #22c55e; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-size: calc(11px * var(--ter-ui-scale)); }

.engine-indicator { 
  font-size: calc(9px * var(--ter-ui-scale)); 
  padding: 2px 6px; 
  border-radius: 4px; 
  background: #18181b; 
  color: #71717a; 
  border: 1px solid #27272a;
  white-space: nowrap;
  display: flex;
  align-items: center;
}
.engine-indicator.native { color: #a855f7; border-color: rgba(168, 85, 247, 0.4); }
</style>
