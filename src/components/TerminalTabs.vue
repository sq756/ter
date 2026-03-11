<script setup lang="ts">
import { WebglAddon } from '@xterm/addon-webgl';

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
}>();

const emit = defineEmits(['switch-tab', 'close-tab', 'new-tab', 'terminal-context']);

// Persistent Terminal Mount Directive (Local logic)
const vMountTerm = {
  mounted: (el: HTMLElement, binding: any) => {
    const tab = binding.value;
    if (tab && tab.instance) {
      console.log(`[UI] Mounting terminal: ${tab.id}`);
      tab.instance.open(el);
      try { tab.instance.loadAddon(new WebglAddon()); } catch (e) {}
      setTimeout(() => tab.fitAddon?.fit(), 50);
    } else {
      console.warn(`[UI] Terminal instance not ready for tab: ${tab?.id}`);
    }
  }
};


const getVisibleTabs = () => props.tabs.filter(t => !t.isBackground);
</script>

<template>
  <div class="terminal-workspace">
    <!-- Multi-Terminal Tab Bar -->
    <nav class="tab-bar">
      <div v-for="t in getVisibleTabs()" :key="t.id" class="tab-item" :class="{ 'active': t.id === activeTabId }" @click="$emit('switch-tab', t.id)">
        <span class="title">{{ t.title }}</span>
        <button class="btn-close" @click.stop="$emit('close-tab', t.id)">×</button>
      </div>
      <button class="btn-new-tab" @click="$emit('new-tab')">+</button>
    </nav>

    <div class="workspace-body">
      <section class="terminal-pane">
        <!-- Persistent Terminal Containers -->
        <div v-for="t in tabs" :key="t.id" 
             v-show="t.id === activeTabId" 
             v-mount-term="t"
             class="terminal-container"
             @click="t.instance?.focus()"
             @contextmenu.prevent="$emit('terminal-context', $event)">
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.terminal-workspace { flex: 1; display: flex; flex-direction: column; height: 100%; overflow: hidden; background: #000; }

.tab-bar { background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; padding: 0 10px; height: 32px; flex-shrink: 0; }
.tab-item { padding: 0 15px; height: 100%; display: flex; align-items: center; font-size: 11px; color: #71717a; border-right: 1px solid #1a1a1c; cursor: pointer; position: relative; min-width: 80px; }
.tab-item.active { background: #1a192f; color: #6366f1; border-top: 2px solid #6366f1; }
.tab-item .btn-close { margin-left: 10px; background: transparent; border: none; color: #444; cursor: pointer; visibility: hidden; font-size: 14px; }
.tab-item:hover .btn-close { visibility: visible; }
.btn-new-tab { background: transparent; border: none; color: #52525b; padding: 0 10px; cursor: pointer; font-size: 18px; line-height: 1; }

.workspace-body { flex: 1; display: flex; overflow: hidden; }
.terminal-pane { flex: 1; padding: 10px; overflow: hidden; position: relative; background: #000; }
.terminal-container { height: 100%; width: 100%; }
</style>
