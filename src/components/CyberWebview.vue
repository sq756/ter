<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { open } from '@tauri-apps/plugin-shell';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AGENT_SCRIPT } from '../constants';

const props = defineProps<{ 
  id: string;
  url: string; 
  isActive: boolean;
  isSafeMode?: boolean;
}>();
const emit = defineEmits(['dom-extracted', 'web-context-menu']);

const containerRef = ref<HTMLElement | null>(null);
const isWebviewReady = ref(false);
const isWebviewError = ref(false);
const isPinned = ref(true);

let unlistenExtracted: any = null;
let resizeObserver: ResizeObserver | null = null;

const WEB_CONTEXT_SCRIPT = `
  window.addEventListener('contextmenu', e => {
    e.preventDefault();
    window.__TAURI__.emit('web-context-menu', { x: e.clientX, y: e.clientY, id: '${props.id}' });
  });
`;

const togglePin = async () => {
  isPinned.value = !isPinned.value;
  await invoke('set_window_always_on_top', { label: props.id, onTop: isPinned.value });
};

const updateWebviewBounds = async () => {
  if (!containerRef.value || !isWebviewReady.value) return;
  await nextTick();
  const rect = containerRef.value.getBoundingClientRect();

  if (props.isActive) {
    await invoke('update_webview_bounds', { 
      label: props.id,
      x: rect.x,
      y: rect.y + 24, // Drag handle offset
      width: rect.width,
      height: rect.height - 24
    }).catch(() => {});
  } else {
    await invoke('update_webview_bounds', { 
      label: props.id,
      x: -10000,
      y: -10000,
      width: 100,
      height: 100
    }).catch(() => {});
  }
};

const initWebview = async () => {
  if (!containerRef.value || props.isSafeMode) return;
  
  isWebviewError.value = false;
  isWebviewReady.value = false;

  const rect = containerRef.value.getBoundingClientRect();

  try {
    await invoke('create_embedded_webview', {
      label: props.id,
      url: props.url,
      x: props.isActive ? rect.x : -10000,
      y: props.isActive ? rect.y + 24 : -10000,
      width: rect.width,
      height: rect.height - 24
    });

    unlistenExtracted = await listen<string>(`dom-extracted-${props.id}`, (ev) => { emit('dom-extracted', ev.payload); });

    isWebviewReady.value = true;
    await invoke('eval_cyber_webview', { label: props.id, code: AGENT_SCRIPT });
    await invoke('eval_cyber_webview', { label: props.id, code: WEB_CONTEXT_SCRIPT });

    if (resizeObserver) resizeObserver.disconnect();
    resizeObserver = new ResizeObserver(() => { updateWebviewBounds(); });
    resizeObserver.observe(containerRef.value);
  } catch (e) {
    console.error("Webview Creation Failed:", e);
    isWebviewError.value = true;
  }
};

const destroyWebview = async () => {
  if (unlistenExtracted) { unlistenExtracted(); unlistenExtracted = null; }
  if (resizeObserver) { resizeObserver.disconnect(); resizeObserver = null; }
  await invoke('close_auth_window').catch(() => {}); 
  isWebviewReady.value = false;
};

const handleRetry = async () => {
  await destroyWebview();
  await nextTick();
  initWebview();
};

watch(() => props.url, (newUrl) => {
  if (isWebviewReady.value) { invoke('navigate_cyber_webview', { label: props.id, url: newUrl }); }
});

watch(() => props.isActive, (active) => { updateWebviewBounds(); });

onMounted(() => { initWebview(); });
onUnmounted(() => { });

const openInBrowser = async () => { try { await open(props.url); } catch(e){} };
defineExpose({ reload: () => invoke('reload_cyber_webview', { label: props.id }), destroy: destroyWebview });
</script>

<template>
  <div class="cyber-webview" ref="containerRef">
    <div class="drag-handle" data-tauri-drag-region>
      <span class="drag-title">{{ url.substring(0, 40) }}...</span>
      <div class="drag-actions">
        <button class="pin-btn" :class="{ 'active': isPinned }" @click="togglePin" title="Toggle Always on Top">📌</button>
      </div>
    </div>

    <div class="native-placeholder" v-if="!isWebviewReady && !isWebviewError">
      <div class="loader">INITIALIZING_TWM_ENGINE...</div>
    </div>
    <div class="native-error" v-if="isWebviewError">
      <div class="error-box">
        <span class="icon">⚠️</span>
        <div class="msg">RENDERER_CRASHED</div>
        <div class="hint">WSL/Linux graphics driver deadlock detected.</div>
        <div class="actions">
          <button class="retry-btn" @click="handleRetry">RETRY_INITIALIZATION</button>
        </div>
      </div>
    </div>
    <div class="tunnel-hint" v-if="url.includes('localhost')">⚡ Agent Injected</div>
    <button class="os-browser-btn" @click="openInBrowser">🌍</button>
  </div>
</template>

<style scoped>
.cyber-webview { display: flex; flex-direction: column; height: 100%; width: 100%; background: #000; position: relative; border: 1px solid #18181b; }
.drag-handle { height: 24px; background: #050505; border-bottom: 1px solid #18181b; display: flex; align-items: center; justify-content: space-between; padding: 0 10px; cursor: move; -webkit-app-region: drag; }
.drag-title { font-size: 9px; color: #52525b; font-family: monospace; pointer-events: none; }
.drag-actions { display: flex; gap: 8px; -webkit-app-region: no-drag; }
.pin-btn { background: transparent; border: none; font-size: 10px; cursor: pointer; opacity: 0.5; transition: all 0.2s; }
.pin-btn.active { opacity: 1; filter: drop-shadow(0 0 5px #22c55e); color: #22c55e; }

.native-placeholder { flex: 1; display: flex; align-items: center; justify-content: center; background: #09090b; color: #a855f7; font-family: 'JetBrains Mono', monospace; font-size: 12px; }
.native-error { flex: 1; display: flex; align-items: center; justify-content: center; background: #09090b; color: #ef4444; font-family: 'JetBrains Mono', monospace; padding: 20px; text-align: center; }
.error-box { border: 1px solid #ef4444; padding: 24px; border-radius: 8px; background: rgba(239, 68, 68, 0.05); max-width: 320px; }
.error-box .msg { font-weight: bold; margin: 10px 0; letter-spacing: 2px; font-size: 14px; }
.error-box .hint { font-size: 10px; opacity: 0.7; margin-bottom: 20px; }
.actions { display: flex; flex-direction: column; gap: 12px; }
.retry-btn { background: #ef4444; color: #000; border: none; padding: 8px 16px; border-radius: 4px; font-weight: bold; cursor: pointer; font-size: 11px; }
.retry-btn:hover { background: #f87171; }
.loader { animation: blink 1s infinite; }
@keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }
.tunnel-hint { position: absolute; bottom: 10px; right: 10px; background: rgba(0, 0, 0, 0.8); color: #a855f7; font-size: 9px; padding: 4px 8px; border-radius: 4px; border: 1px solid rgba(168, 85, 247, 0.3); pointer-events: none; font-family: monospace; z-index: 5; }
.os-browser-btn { position: absolute; top: 34px; right: 10px; background: rgba(0, 0, 0, 0.6); border: 1px solid #27272a; color: #a1a1aa; width: 28px; height: 28px; border-radius: 6px; cursor: pointer; display: flex; align-items: center; justify-content: center; z-index: 10; }
.os-browser-btn:hover { background: #18181b; color: #fff; border-color: #a855f7; }
</style>
