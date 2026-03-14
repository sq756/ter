<script setup lang="ts">
import TerminalView from './TerminalView.vue';
import { terminalManager } from '../TerminalManager';

import { getCurrentWindow } from '@tauri-apps/api/window';
const appWindow = getCurrentWindow();

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
  connectionStatus: 'connected' | 'busy' | 'disconnected';
}>();

defineEmits(['switch-tab', 'close-tab', 'new-tab', 'terminal-context', 'rename-tab', 'pin-tab', 'copy-tab-id']);

const getVisibleTabs = () => props.tabs.filter(t => !t.isBackground);

const minimize = () => appWindow.minimize();
const toggleMaximize = () => appWindow.toggleMaximize();
const closeApp = () => appWindow.close();
</script>

<template>
  <div class="terminal-workspace">
    <!-- Multi-Terminal Tab Bar -->
    <nav class="tab-bar">
      <!-- Status Indicator & Quick Switcher -->
      <div class="status-indicator-zone" @click="$emit('new-tab')">
        <div class="status-dot" :class="connectionStatus"></div>
        <div class="quick-switcher-icon">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
        </div>
      </div>

      <div v-for="t in getVisibleTabs()" 
           :key="t.id" 
           class="tab-item" 
           :class="{ 'active': t.id === activeTabId }" 
           @click="$emit('switch-tab', t.id)"
           @contextmenu.prevent.stop="$emit('terminal-context', { e: $event, id: t.id })">
        <span class="tab-icon">🐚</span>
        <span class="title">{{ t.title }}</span>
        <button class="btn-close" @click.stop="$emit('close-tab', t.id)">×</button>
        <div class="active-bar" v-if="t.id === activeTabId"></div>
      </div>
      <button class="btn-new-tab" @click="$emit('new-tab')">+</button>

      <!-- v2.11.29: Dedicated Drag Region -->
      <div class="drag-region" data-tauri-drag-region></div>

      <!-- v2.11.29: Stealth Window Controls (Isolated) -->
      <div class="window-controls">
        <button class="win-btn" @click="minimize">—</button>
        <button class="win-btn" @click="toggleMaximize">⬜</button>
        <button class="win-btn close" @click="closeApp">✕</button>
      </div>
    </nav>

    <div class="workspace-body">
      <section class="terminal-pane">
        <!-- Persistent Terminal Views: Preserve physical instance with v-show -->
        <div v-for="t in tabs" :key="t.id" 
             class="terminal-wrapper"
             v-show="t.id === activeTabId"
             @click="terminalManager.focus(t.id)"
             @contextmenu.prevent.stop="$emit('terminal-context', { e: $event, id: t.id })">
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
  background: #09090b; 
  position: relative; 
}

.tab-bar { 
  background: #09090b; 
  border-bottom: 1px solid #27272a; 
  display: flex; 
  align-items: center; 
  padding: 0; 
  height: 36px; 
  flex-shrink: 0; 
  z-index: 10; 
  overflow: hidden;
  white-space: nowrap;
}

.status-indicator-zone {
  padding: 0 12px;
  display: flex;
  align-items: center;
  gap: 12px;
  border-right: 1px solid #18181b;
  height: 100%;
  cursor: pointer;
}

.tab-item { 
  padding: 0 16px; 
  height: 100%; 
  display: flex; 
  align-items: center; 
  font-size: 12px; 
  color: #52525b; 
  cursor: pointer; 
  position: relative; 
  min-width: 120px; 
  max-width: 200px;
  transition: all 0.2s; 
  border-right: 1px solid #18181b;
  flex-shrink: 0 !important;
}

.drag-region {
  flex: 1;
  height: 100%;
  cursor: default;
}

.window-controls {
  display: flex;
  height: 100%;
  position: relative;
  z-index: 9999;
}

.win-btn {
  width: 44px;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: #00ff9d;
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
}

.win-btn:hover {
  background: rgba(0, 255, 157, 0.1);
  color: #fff;
}

.win-btn.close:hover {
  background: #ef4444;
}

.btn-new-tab { 
  background: transparent; 
  border: none; 
  color: #52525b; 
  padding: 0 12px; 
  cursor: pointer; 
  font-size: 18px; 
  height: 100%;
}

.btn-new-tab:hover { color: #fff; }

.workspace-body { flex: 1; position: relative; overflow: hidden; display: flex; }
.terminal-pane { height: 100%; flex: 1; position: relative; background: #09090b; }
.terminal-wrapper { position: absolute; inset: 0; width: 100%; height: 100%; }

.status-dot { width: 8px; height: 8px; border-radius: 50%; background: #52525b; }
.status-dot.connected { background: #3b82f6; box-shadow: 0 0 10px #3b82f6; }
.status-dot.busy { background: #a855f7; box-shadow: 0 0 10px #a855f7; }
.quick-switcher-icon { color: #52525b; display: flex; align-items: center; }
.tab-icon { margin-right: 8px; font-size: 12px; opacity: 0.5; }
.tab-item.active { color: #fafafa; background: rgba(255, 255, 255, 0.02); }
.active-bar { position: absolute; bottom: 0; left: 0; width: 100%; height: 2px; background: #3b82f6; }
.tab-item .btn-close { position: absolute; right: 8px; background: transparent; border: none; color: #52525b; cursor: pointer; opacity: 0; }
.tab-item:hover .btn-close { opacity: 1; }
</style>
