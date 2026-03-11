<script setup lang="ts">
import { ref } from 'vue';
import { open } from '@tauri-apps/plugin-shell';

const props = defineProps<{
  initialUrl?: string;
}>();

const previewUrl = ref(props.initialUrl || 'http://localhost:5173');
const iframeRef = ref<HTMLIFrameElement | null>(null);

const reload = () => {
  if (iframeRef.value) {
    iframeRef.value.src = iframeRef.value.src;
  }
};

const goHome = () => {
  previewUrl.value = props.initialUrl || 'http://localhost:5173';
  if (iframeRef.value) iframeRef.value.src = previewUrl.value;
};

const openInBrowser = async () => {
  try {
    await open(previewUrl.value);
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
        <input type="text" :value="previewUrl" readonly />
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
        :src="previewUrl" 
        frameborder="0" 
        allow="cross-origin-isolated"
      ></iframe>
      <div class="tunnel-hint">⚡ Tunneled via SSH (127.0.0.1:5173)</div>
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
  border-left: 1px solid #1a1a1c;
}

.webview-toolbar {
  height: 40px;
  background: rgba(10, 25, 47, 0.8);
  backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  padding: 0 10px;
  gap: 10px;
  border-bottom: 1px solid rgba(99, 102, 241, 0.3);
}

.url-bar {
  flex: 1;
  background: rgba(0, 0, 0, 0.5);
  border: 1px solid rgba(99, 102, 241, 0.2);
  border-radius: 4px;
  display: flex;
  align-items: center;
  padding: 2px 8px;
  box-shadow: inset 0 0 5px rgba(99, 102, 241, 0.1);
}

.url-bar input {
  background: transparent;
  border: none;
  color: #818cf8;
  font-size: 11px;
  width: 100%;
  outline: none;
  font-family: 'JetBrains Mono', monospace;
}

.secure-icon { font-size: 10px; margin-right: 6px; opacity: 0.7; }

.actions { display: flex; gap: 5px; }
.actions button {
  background: transparent;
  border: 1px solid #27272a;
  color: #a1a1aa;
  width: 28px;
  height: 28px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
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
}

iframe {
  width: 100%;
  height: 100%;
  background: #fff; /* Most web pages expect white bg */
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
}
</style>
