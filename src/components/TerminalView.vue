<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { terminalManager } from '../TerminalManager';

const props = defineProps<{
  id: string;
  active: boolean;
  uiScale: number;
}>();

const terminalRef = ref<HTMLElement | null>(null);
let resizeObserver: ResizeObserver | null = null;

const performFit = () => {
  if (props.active && terminalRef.value) {
    const width = terminalRef.value.offsetWidth;
    const height = terminalRef.value.offsetHeight;
    
    if (width > 0 && height > 0) {
      console.log(`[TerminalView:${props.id}] Fitting terminal. Size: ${width}x${height}`);
      const instance = terminalManager.getOrCreate(props.id);
      instance.fit.fit();
      
      const { cols, rows } = instance.term;
      console.log(`[TerminalView:${props.id}] Fit result: ${cols}x${rows}`);
      
      // Sync size with backend PTY
      import('@tauri-apps/api/core').then(({ invoke }) => {
        invoke('resize_pty', { tabId: props.id, cols, rows }).catch(e => {
          console.warn(`[TerminalView:${props.id}] Failed to resize PTY:`, e);
        });
      });
    } else {
      console.debug(`[TerminalView:${props.id}] Skip fit, element hidden or 0 size`);
    }
  }
};

const initTerminal = async (retries = 5) => {
  if (!terminalRef.value) {
    if (retries > 0) setTimeout(() => initTerminal(retries - 1), 100);
    return;
  }
  
  try {
    terminalManager.mount(props.id, terminalRef.value);
    
    // Stabilize layout
    setTimeout(performFit, 100);
    setTimeout(performFit, 500);
    setTimeout(performFit, 1000);
  } catch (e) {
    console.error(`[TerminalView:${props.id}] Mount failed, retrying...`, e);
    if (retries > 0) {
      setTimeout(() => initTerminal(retries - 1), 200);
      return;
    }
  }

  const instance = terminalManager.getOrCreate(props.id);

  if (resizeObserver) resizeObserver.disconnect();
  resizeObserver = new ResizeObserver(() => {
    performFit();
  });
  resizeObserver.observe(terminalRef.value);

  if (props.active) {
    instance.term.focus();
  }
};

let unlistenDirect: any = null;

onMounted(async () => {
  initTerminal();
  unlistenDirect = await listen(`pty-data-${props.id}`, (event: any) => {
    terminalManager.write(props.id, event.payload);
  });
});

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect();
  }
  if (unlistenDirect) unlistenDirect();
});

// v2.11.53: Real-time Terminal Scaling
watch(() => props.uiScale, (newScale) => {
  const instance = terminalManager.getOrCreate(props.id);
  if (instance) {
    instance.term.options.fontSize = 14 * newScale;
    // Debounced fit via nextTick/requestAnimationFrame
    nextTick(() => {
      performFit();
    });
  }
});

watch(() => props.active, async (isActive) => {
  if (isActive) {
    console.log(`[TerminalView:${props.id}] Tab became active`);
    await nextTick(); 
    
    // Extra stabilize for Linux rendering
    requestAnimationFrame(() => {
      performFit();
      terminalManager.focus(props.id);
    });
    
    // One more try after a short delay
    setTimeout(performFit, 300);
  }
});
</script>

<template>
  <div ref="terminalRef" class="terminal-view-container" @contextmenu.prevent.stop="$emit('terminal-context', { e: $event, id: props.id })"></div>
</template>

<style>
.terminal-view-container {
  width: 100%;
  height: 100%;
  min-height: 100px;
  min-width: 100px;
  background: #000;
  overflow: hidden;
  position: relative;
  /* Ensure it has a block layout */
  display: block;
  /* v2.11.56: Rendering Isolation */
  contain: content;
  will-change: transform;
}

.terminal-view-container .xterm {
  padding: 10px;
  height: 100%;
  width: 100%;
}

.terminal-view-container .xterm-viewport {
  background-color: #000 !important;
}

/* Fix xterm background transparency */
.xterm-screen {
  background-color: #09090b !important;
}
</style>
