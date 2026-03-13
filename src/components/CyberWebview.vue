<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { open } from '@tauri-apps/plugin-shell';
import { Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AGENT_SCRIPT } from '../constants';

const props = defineProps<{ url: string; }>();
const emit = defineEmits(['dom-extracted']);

const containerRef = ref<HTMLElement | null>(null);
let webview: Webview | null = null;
const isWebviewReady = ref(false);
let unlistenExtracted: any = null;
let resizeObserver: ResizeObserver | null = null;

const updateWebviewBounds = async () => {
  if (!webview || !containerRef.value) return;
  await nextTick();
  const rect = containerRef.value.getBoundingClientRect();
  const dpr = window.devicePixelRatio;

  // v2.11.11: Use Math.floor and precise coordinates
  await webview.setSize({
    type: 'Physical',
    width: Math.floor(rect.width * dpr),
    height: Math.floor(rect.height * dpr),
  });
  await webview.setPosition({
    type: 'Physical',
    x: Math.floor(rect.left * dpr),
    y: Math.floor(rect.top * dpr),
  });
};

const initWebview = async () => {
  if (!containerRef.value || webview) return;
  
  const currentWin = getCurrentWindow();
  const rect = containerRef.value.getBoundingClientRect();
  const dpr = window.devicePixelRatio;

  webview = new Webview(currentWin, 'cyber-native-view', {
    url: props.url,
    x: Math.floor(rect.left * dpr),
    y: Math.floor(rect.top * dpr),
    width: Math.floor(rect.width * dpr),
    height: Math.floor(rect.height * dpr),
  });

  unlistenExtracted = await listen<string>('dom-extracted', (ev) => { emit('dom-extracted', ev.payload); });

  webview.once('tauri://created', async () => {
    isWebviewReady.value = true;
    await invoke('eval_cyber_webview', { code: AGENT_SCRIPT });
  });

  // Setup ResizeObserver for coordinate sync
  resizeObserver = new ResizeObserver(() => {
    updateWebviewBounds();
  });
  resizeObserver.observe(containerRef.value);
};

const destroyWebview = async () => {
  if (unlistenExtracted) {
    unlistenExtracted();
    unlistenExtracted = null;
  }
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
  if (webview) {
    await webview.close();
    webview = null;
    isWebviewReady.value = false;
  }
};

watch(() => props.url, (newUrl) => {
  if (isWebviewReady.value) {
    invoke('navigate_cyber_webview', { url: newUrl });
  }
});

onMounted(() => {
  initWebview();
});

onUnmounted(() => {
  destroyWebview();
});

const openInBrowser = async () => { try { await open(props.url); } catch(e){} };
defineExpose({ reload: () => invoke('reload_cyber_webview') });
</script>

<template>
  <div class="cyber-webview" ref="containerRef">
    <div class="native-placeholder" v-if="!isWebviewReady"><div class="loader">INITIALIZING_AGENTIC_WEBVIEW...</div></div>
    <div class="tunnel-hint" v-if="url.includes('localhost')">⚡ Agent Injected</div>
    <button class="os-browser-btn" @click="openInBrowser">🌍</button>
  </div>
</template>

<style scoped>
.cyber-webview { display: flex; flex-direction: column; height: 100%; width: 100%; background: #000; position: relative; }
.native-placeholder { flex: 1; display: flex; align-items: center; justify-content: center; background: #09090b; color: #a855f7; font-family: 'JetBrains Mono', monospace; font-size: 12px; }
.loader { animation: blink 1s infinite; }
@keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
.tunnel-hint { position: absolute; bottom: 10px; right: 10px; background: rgba(0, 0, 0, 0.8); color: #a855f7; font-size: 9px; padding: 4px 8px; border-radius: 4px; border: 1px solid rgba(168, 85, 247, 0.3); pointer-events: none; font-family: monospace; z-index: 5; }
.os-browser-btn { position: absolute; top: 10px; right: 10px; background: rgba(0, 0, 0, 0.6); border: 1px solid #27272a; color: #a1a1aa; width: 28px; height: 28px; border-radius: 6px; cursor: pointer; display: flex; align-items: center; justify-content: center; z-index: 10; }
.os-browser-btn:hover { background: #18181b; color: #fff; border-color: #a855f7; }
</style>
