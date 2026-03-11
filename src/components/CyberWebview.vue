<script setup lang="ts">
import { ref } from 'vue';
import { open } from '@tauri-apps/plugin-shell';

const props = defineProps<{
  url: string;
}>();

const iframeRef = ref<HTMLIFrameElement | null>(null);

const reload = () => {
  if (iframeRef.value) {
    iframeRef.value.src = iframeRef.value.src;
  }
};

const goHome = () => {
  if (iframeRef.value) iframeRef.value.src = props.url;
};

const openInBrowser = async () => {
  try {
    await open(props.url);
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
        <input type="text" :value="url" readonly />
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
        :src="url" 
        frameborder="0" 
        allow="cross-origin-isolated"
      ></iframe>
      <div class="tunnel-hint">⚡ Tunneled via SSH ({{ url }})</div>
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
  height: 32px;
  background: rgba(10, 25, 47, 0.9);
  backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  padding: 0 8px;
  gap: 8px;
  border-bottom: 1px solid rgba(99, 102, 241, 0.3);
}

.url-bar {
  flex: 1;
  background: rgba(0, 0, 0, 0.5);
  border: 1px solid rgba(99, 102, 241, 0.2);
  border-radius: 4px;
  display: flex;
  align-items: center;
  padding: 1px 8px;
  box-shadow: inset 0 0 5px rgba(99, 102, 241, 0.1);
}

.url-bar input {
  background: transparent;
  border: none;
  color: #818cf8;
  font-size: 10px;
  width: 100%;
  outline: none;
  font-family: 'JetBrains Mono', monospace;
}

.secure-icon { font-size: 10px; margin-right: 6px; opacity: 0.7; }

.actions { display: flex; gap: 4px; }
.actions button {
  background: transparent;
  border: 1px solid #27272a;
  color: #a1a1aa;
  width: 24px;
  height: 24px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: 0.2s;
}

.actions button:hover {
  background: rgba(99, 102, 241, 0.2);
  border-color: #6366f1;
  color: #fff;
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
