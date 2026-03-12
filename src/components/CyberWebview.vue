<script setup lang="ts">
import { ref, watch } from 'vue';
import { open } from '@tauri-apps/plugin-shell';

const props = defineProps<{
  url: string;
}>();

const iframeRef = ref<HTMLIFrameElement | null>(null);
const currentDisplayUrl = ref(props.url);

watch(() => props.url, (newUrl) => {
  currentDisplayUrl.value = newUrl;
});

const reload = () => {
  if (iframeRef.value) {
    iframeRef.value.src = iframeRef.value.src;
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
    <div class="iframe-container">
      <iframe 
        ref="iframeRef" 
        :src="currentDisplayUrl" 
        frameborder="0" 
        allow="cross-origin-isolated"
      ></iframe>
      <div class="tunnel-hint" v-if="currentDisplayUrl.includes('localhost')">⚡ SSH Tunnel Active</div>
      <button class="os-browser-btn" @click="openInBrowser" title="Open in System Browser">🌍</button>
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

.iframe-container {
  flex: 1;
  position: relative;
  overflow: hidden;
  background: #000;
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

.os-browser-btn {
  position: absolute;
  top: 10px;
  right: 10px;
  background: rgba(0, 0, 0, 0.6);
  border: 1px solid #27272a;
  color: #a1a1aa;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
  transition: all 0.2s;
}

.os-browser-btn:hover {
  background: #18181b;
  color: #fff;
  border-color: #3b82f6;
}
</style>
