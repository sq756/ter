<script setup lang="ts">
import { terminalManager } from '../TerminalManager';

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
}>();

const emit = defineEmits(['switch-tab', 'close-tab', 'new-tab', 'terminal-context']);

/**
 * Custom directive to bridge Vue lifecycle with xterm.js non-reactive instances.
 * Using 'v-attach-term' instead of old 'v-mount-term' for the new architecture.
 */
const vAttachTerm = {
  mounted: (el: HTMLElement, binding: any) => {
    const tabId = binding.value;
    if (tabId) {
      console.log(`[UI] v-attach-term: Attaching terminal ${tabId}`);
      
      terminalManager.getOrCreate(tabId);
      terminalManager.mount(tabId, el);

      // The Golden Delay: Ensure layout is settled before fitting
      requestAnimationFrame(() => {
        setTimeout(() => terminalManager.fit(tabId), 50);
        setTimeout(() => terminalManager.fit(tabId), 150);
      });

      // Simple debounce for ResizeObserver
      let resizeTimeout: any = null;
      const ro = new ResizeObserver((entries) => {
        for (let entry of entries) {
          if (entry.contentRect.width > 0 && entry.contentRect.height > 0) {
            console.log(`[DEBUG] Terminal ${tabId} Resized: ${entry.contentRect.width}x${entry.contentRect.height}`);
            
            // Immediate fit for responsiveness
            terminalManager.fit(tabId);

            // Debounced secondary fit to catch final layout settles
            if (resizeTimeout) clearTimeout(resizeTimeout);
            resizeTimeout = setTimeout(() => {
              terminalManager.fit(tabId);
            }, 50);
          }
        }
      });
      ro.observe(el);
      (el as any)._ro = ro;
    }
  },
  unmounted: (el: HTMLElement) => {
    if ((el as any)._ro) {
      (el as any)._ro.disconnect();
    }
  }
};

const getVisibleTabs = () => props.tabs.filter(t => !t.isBackground);
</script>

<template>
  <div class="terminal-workspace">
    <!-- Multi-Terminal Tab Bar -->
    <nav class="tab-bar">
      <div v-for="t in getVisibleTabs()" 
           :key="t.id" 
           class="tab-item" 
           :class="{ 'active': t.id === activeTabId }" 
           @click="$emit('switch-tab', t.id)">
        <span class="title">{{ t.title }}</span>
        <button class="btn-close" @click.stop="$emit('close-tab', t.id)">×</button>
      </div>
      <button class="btn-new-tab" @click="$emit('new-tab')">+</button>
    </nav>

    <div class="workspace-body">
      <section class="terminal-pane">
        <!-- Persistent Terminal Containers: Preserve physical size with visibility: hidden -->
        <div v-for="t in tabs" :key="t.id" 
             :class="['terminal-container', { 'inactive-tab': t.id !== activeTabId }]"
             v-attach-term="t.id"
             @contextmenu.prevent="$emit('terminal-context', { e: $event, id: t.id })">
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.terminal-workspace { flex: 1; display: flex; flex-direction: column; height: 100%; overflow: hidden; background: #000; position: relative; }

.tab-bar { background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; padding: 0 10px; height: 32px; flex-shrink: 0; z-index: 10; }
.tab-item { padding: 0 15px; height: 100%; display: flex; align-items: center; font-size: 11px; color: #71717a; border-right: 1px solid #1a1a1c; cursor: pointer; position: relative; min-width: 80px; }
.tab-item.active { background: #1a192f; color: #6366f1; border-top: 2px solid #6366f1; }
.tab-item .btn-close { margin-left: 10px; background: transparent; border: none; color: #444; cursor: pointer; visibility: hidden; font-size: 14px; }
.tab-item:hover .btn-close { visibility: visible; }
.btn-new-tab { background: transparent; border: none; color: #52525b; padding: 0 10px; cursor: pointer; font-size: 18px; line-height: 1; }

.workspace-body { flex: 1; position: relative; overflow: hidden; }
.terminal-pane { height: 100%; width: 100%; position: relative; background: #000; }

/* 
 * PHYSICAL-INSET: Absolute positioning ensures zero jitter from Flexbox 
 * and ensures the container always fills its parent 1:1.
 */
.terminal-container { 
  position: absolute;
  inset: 0;
  overflow: hidden; 
  min-width: 100px;
  min-height: 100px;
  pointer-events: auto !important;
}

/* PHYSICAL-KEEP-ALIVE: visibility: hidden keeps the layout metrics alive */
.inactive-tab {
  visibility: hidden;
  pointer-events: none;
  z-index: -1;
}

/* Force xterm internal elements to fill container */
:deep(.xterm), 
:deep(.xterm-viewport), 
:deep(.xterm-screen),
:deep(canvas) {
  display: block !important;
  width: 100% !important;
  height: 100% !important;
  background-color: #000 !important;
}
</style>
