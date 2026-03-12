<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue';
import { open } from '@tauri-apps/plugin-shell';
import { Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  url: string;
}>();

const containerRef = ref<HTMLElement | null>(null);
let webview: Webview | null = null;
const isWebviewReady = ref(false);

const syncWebviewBounds = async () => {
  if (!webview || !containerRef.value) return;
  
  const rect = containerRef.value.getBoundingClientRect();
  
  // v2.3.0: Bounds sync logic
  await webview.setSize(new PhysicalSize(
    Math.round(rect.width * window.devicePixelRatio),
    Math.round(rect.height * window.devicePixelRatio)
  ));
  
  await webview.setPosition(new PhysicalPosition(
    Math.round(rect.left * window.devicePixelRatio),
    Math.round(rect.top * window.devicePixelRatio)
  ));
};

onMounted(async () => {
  if (!containerRef.value) return;

  const currentWin = getCurrentWindow();
  const rect = containerRef.value.getBoundingClientRect();

  // Create Native Webview
  webview = new Webview(currentWin, 'cyber-native-view', {
    url: props.url,
    x: Math.round(rect.left * window.devicePixelRatio),
    y: Math.round(rect.top * window.devicePixelRatio),
    width: Math.round(rect.width * window.devicePixelRatio),
    height: Math.round(rect.height * window.devicePixelRatio),
  });

  webview.once('tauri://created', () => {
    isWebviewReady.value = true;
    console.log("[CyberWebview] Native Webview Created");
  });

  webview.once('tauri://error', (e) => {
    console.error("[CyberWebview] Failed to create Webview:", e);
  });

  // Track size/position changes
  const resizeObserver = new ResizeObserver(() => syncWebviewBounds());
  resizeObserver.observe(containerRef.value);

  onUnmounted(async () => {
    resizeObserver.disconnect();
    if (webview) {
      await webview.close();
      webview = null;
    }
  });
});

watch(() => props.url, async (newUrl) => {
  if (isWebviewReady.value) {
    // v2.3.0: Update location via Rust Bridge
    await invoke('navigate_cyber_webview', { url: newUrl });
  }
});

const reload = async () => {
  if (isWebviewReady.value) {
    await invoke('reload_cyber_webview');
  }
};

const openInBrowser = async () => {
  try {
    await open(props.url);
  } catch (e) {
    console.error("Failed to open system browser:", e);
  }
};

defineExpose({ reload });
</script>

<template>
  <div class="cyber-webview" ref="containerRef">
    <div class="native-placeholder" v-if="!isWebviewReady">
      <div class="loader">INITIALIZING_NATIVE_WEBVIEW...</div>
    </div>
    <div class="tunnel-hint" v-if="url.includes('localhost')">⚡ Native Proxy Active</div>
    <button class="os-browser-btn" @click="openInBrowser" title="Open in System Browser">🌍</button>
  </div>
</template>

<style scoped>
.cyber-webview {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background: #000;
  position: relative;
}

.native-placeholder {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #09090b;
  color: #22c55e;
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
}

.loader {
  animation: blink 1s infinite;
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
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