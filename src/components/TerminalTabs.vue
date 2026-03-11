<script setup lang="ts">
import TerminalView from './TerminalView.vue';

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
}>();

defineEmits(['switch-tab', 'close-tab', 'new-tab', 'terminal-context']);

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
        <!-- Persistent Terminal Views: Preserve physical instance with v-show -->
        <div v-for="t in tabs" :key="t.id" 
             class="terminal-wrapper"
             v-show="t.id === activeTabId"
             @contextmenu.prevent="$emit('terminal-context', { e: $event, id: t.id })">
          <TerminalView :id="t.id" :active="t.id === activeTabId" />
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.terminal-workspace { 
  flex: 1; 
  display: flex; 
  flex-direction: column; 
  height: 100%; 
  overflow: hidden; 
  background: #000; 
  position: relative; 
}

.tab-bar { background: #0c0c0e; border-bottom: 1px solid #1a1a1c; display: flex; align-items: center; padding: 0 10px; height: 32px; flex-shrink: 0; z-index: 10; }
.tab-item { padding: 0 15px; height: 100%; display: flex; align-items: center; font-size: 11px; color: #71717a; border-right: 1px solid #1a1a1c; cursor: pointer; position: relative; min-width: 80px; }
.tab-item.active { background: #1a192f; color: #6366f1; border-top: 2px solid #6366f1; }
.tab-item .btn-close { margin-left: 10px; background: transparent; border: none; color: #444; cursor: pointer; visibility: hidden; font-size: 14px; }
.tab-item:hover .btn-close { visibility: visible; }
.btn-new-tab { background: transparent; border: none; color: #52525b; padding: 0 10px; cursor: pointer; font-size: 18px; line-height: 1; }

.workspace-body { flex: 1; position: relative; overflow: hidden; display: flex; }
.terminal-pane { 
  height: 100%; 
  flex: 1;
  position: relative; 
  background: #000; 
}

.terminal-wrapper {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}
</style>
