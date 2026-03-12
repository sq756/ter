<script setup lang="ts">
import { ref, watch } from 'vue';
import { open } from '@tauri-apps/plugin-shell';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  url: string;
}>();

const iframeRef = ref<HTMLIFrameElement | null>(null);
const internalUrl = ref(props.url);
const currentDisplayUrl = ref(props.url);

watch(() => props.url, (newUrl) => {
  internalUrl.value = newUrl;
  currentDisplayUrl.value = newUrl;
});

const reload = () => {
  if (iframeRef.value) {
    iframeRef.value.src = iframeRef.value.src;
  }
};

const goHome = () => {
  currentDisplayUrl.value = props.url;
  internalUrl.value = props.url;
  if (iframeRef.value) iframeRef.value.src = props.url;
};

const handleUrlEnter = async () => {
  const urlStr = internalUrl.value.trim();
  if (!urlStr) return;

  // Pattern: localhost:PORT or 127.0.0.1:PORT
  const match = urlStr.match(/(?:localhost|127\.0\.0\.1):(\d+)/);
  if (match && match[1]) {
    const remotePort = parseInt(match[1]);
    try {
      console.log(`[CyberView] Requesting dynamic tunnel for port ${remotePort}...`);
      const localPort = await invoke<number>('open_dynamic_tunnel', { remotePort });
      const newLocalUrl = `http://localhost:${localPort}`;
      console.log(`[CyberView] Tunnel established: ${newLocalUrl}`);
      currentDisplayUrl.value = newLocalUrl;
      if (iframeRef.value) iframeRef.value.src = newLocalUrl;
    } catch (e) {
      console.error("Failed to open dynamic tunnel:", e);
      alert(`Tunnel failed: ${e}`);
    }
  } else {
    // Regular URL
    currentDisplayUrl.value = urlStr;
    if (iframeRef.value) iframeRef.value.src = urlStr;
  }
};

const openInBrowser = async () => {
  try {
    await open(currentDisplayUrl.value);
  } catch (e) {
    console.error("Failed to open system browser:", e);
  }
};

// Expose refresh for parent (RPC)
defineExpose({ reload });
</script>

<template>
  <div class="cyber-webview">
    <nav class="webview-toolbar">
      <div class="url-bar">
        <span class="secure-icon">🔒</span>
        <input 
          type="text" 
          v-model="internalUrl" 
          @keyup.enter="handleUrlEnter"
          placeholder="Enter URL (e.g. localhost:8080)"
        />
      </div>
      <div class="actions">
        <button @click="reload" title="Reload (Ctrl+R)">🔄</button>
        <button @click="goHome" title="Home">🏠</button>
        <button @click="openInBrowser" title="Open in OS Browser">🌍</button>
      </div>
    </nav>
    <div class="iframe-container">
      <iframe 
        ref="iframeRef" 
        :src="currentDisplayUrl" 
        frameborder="0" 
        allow="cross-origin-isolated"
      ></iframe>
      <div class="tunnel-hint">⚡ Tunneled via SSH ({{ internalUrl }})</div>
    </div>
  </div>
</template>

<style scoped>
.cyber-webview {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background: #000;
}

.webview-toolbar {
  height: 40px;
  background: #09090b;
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 12px;
  border-bottom: 1px solid #27272a;
}

.url-bar {
  flex: 1;
  background: #18181b;
  border: 1px solid #27272a;
  border-radius: 8px;
  display: flex;
  align-items: center;
  padding: 4px 12px;
  transition: border-color 0.2s;
}

.url-bar:focus-within {
  border-color: #3f3f46;
}

.url-bar input {
  background: transparent;
  border: none;
  color: #a1a1aa;
  font-size: 11px;
  width: 100%;
  outline: none;
  font-family: 'JetBrains Mono', monospace;
}

.secure-icon { font-size: 10px; margin-right: 8px; opacity: 0.5; }

.actions { display: flex; gap: 6px; }
.actions button {
  background: transparent;
  border: none;
  color: #71717a;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.actions button:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fafafa;
}

.iframe-container {
  flex: 1;
  position: relative;
  overflow: hidden;
  background: #000; /* Prevent white flash */
}

iframe {
  width: 100%;
  height: 100%;
  background: #fff;
}

.tunnel-hint {
  position: absolute;
  bottom: 10px;
  right: 10px;
  background: rgba(0, 0, 0, 0.8);
  color: #22c55e;
  font-size: 9px;
  padding: 4px 8px;
  border-radius: 4px;
  border: 1px solid rgba(34, 197, 94, 0.3);
  pointer-events: none;
  font-family: monospace;
  z-index: 5;
}
</style>
