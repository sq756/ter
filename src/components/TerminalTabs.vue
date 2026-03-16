<script setup lang="ts">
import { ref, nextTick } from 'vue';
import TerminalView from './TerminalView.vue';
import TerEditor from './TerEditor.vue';
import CyberPdfViewer from './CyberPdfViewer.vue';
import { terminalManager } from '../TerminalManager';

import { getCurrentWindow } from '@tauri-apps/api/window';
const appWindow = getCurrentWindow();

import { globalState } from '../store';

const props = defineProps<{
  tabs: any[];
  activeTabId: string | null;
  activeTabIdSecondary: string | null;
  splitMode: boolean;
  connectionStatus: 'connected' | 'busy' | 'disconnected';
  uiScale: number;
}>();

const emit = defineEmits(['switch-tab', 'switch-tab-secondary', 'close-tab', 'new-tab', 'terminal-context', 'rename-tab', 'pin-tab', 'copy-tab-id', 'toggle-split', 'save-complete']);

const splitVertical = ref(localStorage.getItem('ter_split_vertical') === 'true');

const getVisibleTabs = () => props.tabs.filter(t => !t.isBackground);

const minimize = () => appWindow.minimize();
const toggleMaximize = () => appWindow.toggleMaximize();
const closeApp = () => appWindow.close();

const switchTab = (id: string) => {
  emit('switch-tab', id);
  globalState.focusedPane = 'primary';
  nextTick(() => {
    terminalManager.focus(id);
  });
};

const switchTabSecondary = (id: string) => {
  emit('switch-tab-secondary', id);
  globalState.focusedPane = 'secondary';
  nextTick(() => {
    terminalManager.focus(id);
  });
};

const toggleSplitDirection = () => {
  splitVertical.value = !splitVertical.value;
  localStorage.setItem('ter_split_vertical', splitVertical.value.toString());
  nextTick(() => {
    window.dispatchEvent(new Event('resize'));
  });
};

const handleTabWheel = (e: WheelEvent) => {
  if (e.shiftKey) {
    e.preventDefault();
    const visibleTabs = getVisibleTabs();
    if (visibleTabs.length <= 1) return;
    
    const currentIndex = visibleTabs.findIndex(t => t.id === props.activeTabId);
    if (currentIndex === -1) return;
    
    const nextIndex = e.deltaY > 0 
      ? (currentIndex + 1) % visibleTabs.length
      : (currentIndex - 1 + visibleTabs.length) % visibleTabs.length;
    
    switchTab(visibleTabs[nextIndex].id);
  }
};
</script>

<template>
  <div class="terminal-workspace">
    <!-- Multi-Terminal Tab Bar -->
    <nav class="tab-bar" @wheel="handleTabWheel">
      <div class="tab-bar-content">
        <!-- Status Indicator & Quick Switcher -->
        <div class="status-indicator-zone" @click="$emit('new-tab')">
          <div class="status-dot" :class="connectionStatus"></div>
          <div class="quick-switcher-icon">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
          </div>
        </div>

        <div class="tabs-scroll-area">
          <div v-for="t in getVisibleTabs()" 
               :key="t.id" 
               class="tab-item" 
               :class="{ 'active': t.id === activeTabId || (splitMode && t.id === activeTabIdSecondary) }" 
               @click.stop="splitMode && activeTabId ? switchTabSecondary(t.id) : switchTab(t.id)"
               @contextmenu.prevent.stop="$emit('terminal-context', { e: $event, id: t.id })">
            <span class="tab-icon">
              <svg v-if="t.viewType === 'terminal'" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line></svg>
              <svg v-else-if="t.viewType === 'webview'" viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10z"></path></svg>
              <svg v-else viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline></svg>
            </span>
            <span class="title">{{ t.title }}</span>
            <button class="btn-close" @click.stop="$emit('close-tab', t.id)">
              <svg viewBox="0 0 24 24" width="10" height="10" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
            </button>
            <div class="active-bar" v-if="t.id === activeTabId"></div>
            <div class="active-bar-secondary" v-if="splitMode && t.id === activeTabIdSecondary"></div>
          </div>
        </div>

        <button class="btn-new-tab" @click.stop="$emit('new-tab')" title="New Tab">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
        </button>
        <button class="btn-split" 
                :class="{ 'active': splitMode, 'vertical': splitVertical }" 
                @click.stop="$emit('toggle-split')" 
                @contextmenu.prevent="toggleSplitDirection"
                title="Toggle Split (Right-click for direction)">
          <svg v-if="!splitVertical" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><line x1="12" y1="3" x2="12" y2="21"></line></svg>
          <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><line x1="3" y1="12" x2="21" y2="12"></line></svg>
        </button>

        <!-- v2.14.22: Re-enabling pure drag spacer to fix interaction dead zones -->
        <div class="drag-spacer" data-tauri-drag-region></div>

        <!-- v2.11.29: Stealth Window Controls (Isolated) -->
        <div class="window-controls">
          <button class="win-btn" @click.stop="minimize">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="5" y1="12" x2="19" y2="12"></line></svg>
          </button>
          <button class="win-btn" @click.stop="toggleMaximize">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect></svg>
          </button>
          <button class="win-btn close" @click.stop="closeApp">
            <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
          </button>
        </div>
      </div>
    </nav>

    <div class="workspace-body" :class="{ 'split-mode': splitMode, 'split-vertical': splitVertical }">
      <!-- Primary Pane -->
      <section class="pane primary-pane" :class="{ 'active-pane': activeTabId && globalState.focusedPane === 'primary' }">
        <div v-if="activeTabId" 
             class="tab-view-wrapper"
             @mousedown="globalState.focusedPane = 'primary'; terminalManager.focus(activeTabId)">
          <!-- v2.15.28: Slot-based rendering to prevent DOM thrashing -->
          <template v-for="t in [tabs.find(x => x.id === activeTabId)]" :key="'prim-active-' + activeTabId">
            <TerminalView v-if="t && t.viewType === 'terminal'" 
                          :id="t.id" 
                          :active="true" 
                          :uiScale="uiScale"
                          @terminal-context="$emit('terminal-context', $event)" />
            <TerEditor v-else-if="t && t.viewType === 'editor'" 
                       :id="t.id" 
                       :path="t.data?.path" 
                       :initialContent="t.data?.content"
                       @save-complete="$emit('save-complete')" />
            <CyberPdfViewer v-else-if="t && t.viewType === 'webview'" 
                            :url="t.data?.url" 
                            :title="t.title" />
          </template>
        </div>
        <div v-if="!activeTabId" class="empty-pane-msg">SELECT_TAB_FOR_DECK_1</div>
      </section>

      <!-- Secondary Pane -->
      <section v-if="splitMode" class="pane secondary-pane" :class="{ 'active-pane-sec': activeTabIdSecondary && globalState.focusedPane === 'secondary' }">
        <div v-if="activeTabIdSecondary" 
             class="tab-view-wrapper"
             @mousedown="globalState.focusedPane = 'secondary'; terminalManager.focus(activeTabIdSecondary)">
          <!-- v2.15.28: Slot-based rendering for secondary pane -->
          <template v-for="t in [tabs.find(x => x.id === activeTabIdSecondary)]" :key="'sec-active-' + activeTabIdSecondary">
            <TerminalView v-if="t && t.viewType === 'terminal'" 
                          :id="t.id" 
                          :active="true" 
                          :uiScale="uiScale"
                          @terminal-context="$emit('terminal-context', $event)" />
            <TerEditor v-else-if="t && t.viewType === 'editor'" 
                       :id="t.id" 
                       :path="t.data?.path" 
                       :initialContent="t.data?.content"
                       @save-complete="$emit('save-complete')" />
            <CyberPdfViewer v-else-if="t && t.viewType === 'webview'" 
                            :url="t.data?.url" 
                            :title="t.title" />
          </template>
        </div>
        <div v-if="!activeTabIdSecondary" class="empty-pane-msg">SELECT_TAB_FOR_DECK_2</div>
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
  height: calc(var(--ter-tab-bar-height) * var(--ter-ui-scale)); 
  flex-shrink: 0; 
  z-index: 10; 
  overflow: hidden;
}

.tab-bar-content {
  position: relative;
  z-index: 100 !important;
  display: flex;
  align-items: center;
  width: 100%;
  height: 100%;
}


.status-indicator-zone {
  padding: 0 calc(12px * var(--ter-ui-scale));
  display: flex;
  align-items: center;
  gap: calc(12px * var(--ter-ui-scale));
  border-right: 1px solid #18181b;
  height: 100%;
  cursor: pointer;
  flex-shrink: 0;
  position: relative;
  z-index: 101 !important;
}

.tabs-scroll-area {
  display: flex;
  height: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  flex: 0 1 auto;
  scrollbar-width: none;
  position: relative;
  z-index: 101 !important;
}
.tabs-scroll-area::-webkit-scrollbar { display: none; }

.tab-item { 
  padding: 0 calc(16px * var(--ter-ui-scale)); 
  height: 100%; 
  display: flex; 
  align-items: center; 
  font-size: calc(12px * var(--ter-ui-scale)); 
  color: #52525b; 
  cursor: pointer; 
  position: relative; 
  min-width: calc(100px * var(--ter-ui-scale)); 
  max-width: calc(240px * var(--ter-ui-scale));
  transition: all 0.1s; 
  border-right: 1px solid #18181b;
  flex-shrink: 0 !important;
  white-space: nowrap;
  z-index: 20; 
}

.btn-new-tab, .btn-split, .win-btn {
  position: relative;
  z-index: 120 !important;
  pointer-events: auto !important;
}

.drag-spacer {
  flex: 1;
  height: 100%;
  pointer-events: none !important;
}

.window-controls {
  display: flex;
  height: 100%;
  position: relative;
  z-index: 10;
  flex-shrink: 0;
}

.win-btn {
  width: calc(44px * var(--ter-ui-scale));
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  color: #00ff9d;
  cursor: pointer;
  font-size: calc(12px * var(--ter-ui-scale));
  transition: all 0.1s;
  text-shadow: 0 0 calc(4px * var(--ter-ui-scale)) rgba(0, 255, 157, 0.4);
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

.btn-split { background: transparent; border: none; color: #52525b; cursor: pointer; padding: 0 12px; height: 100%; transition: all 0.1s; border-left: 1px solid #18181b; }
.btn-split:hover { color: #fff; }
.btn-split.active { color: #22c55e; background: rgba(34, 197, 94, 0.05); }
.btn-split.vertical { color: #3b82f6; background: rgba(59, 130, 246, 0.05); }

.workspace-body { flex: 1; position: relative; overflow: hidden; display: flex; width: 100%; height: 100%; }
.workspace-body.split-mode { flex-direction: row; }
.workspace-body.split-mode.split-vertical { flex-direction: column; }

.pane { flex: 1; height: 100%; position: relative; overflow: hidden; min-width: 0; transition: all 0.3s ease; border: 2px solid transparent; }
.primary-pane { background: #000; }
.secondary-pane { border-left: 1px solid #27272a; background: #000; }
.split-vertical .secondary-pane { border-left: none; border-top: 1px solid #27272a; }

.active-pane { border-color: rgba(59, 130, 246, 0.5); animation: glow-blue 3s infinite ease-in-out; }
.active-pane-sec { border-color: rgba(168, 85, 247, 0.5); animation: glow-purple 3s infinite ease-in-out; }

@keyframes glow-blue {
  0%, 100% { box-shadow: inset 0 0 15px rgba(59, 130, 246, 0.1); border-color: rgba(59, 130, 246, 0.3); }
  50% { box-shadow: inset 0 0 25px rgba(59, 130, 246, 0.3); border-color: rgba(59, 130, 246, 0.6); }
}

@keyframes glow-purple {
  0%, 100% { box-shadow: inset 0 0 15px rgba(168, 85, 247, 0.1); border-color: rgba(168, 85, 247, 0.3); }
  50% { box-shadow: inset 0 0 25px rgba(168, 85, 247, 0.3); border-color: rgba(168, 85, 247, 0.6); }
}

.tab-view-wrapper { position: absolute; inset: 0; width: 100%; height: 100%; }

.empty-pane-msg { height: 100%; display: flex; align-items: center; justify-content: center; color: #3f3f46; font-size: 10px; font-family: 'JetBrains Mono', monospace; letter-spacing: 2px; }

.status-dot { 
  width: 8px; 
  height: 8px; 
  border-radius: 50%; 
  background: #52525b; 
  position: relative;
  transition: all 0.1s;
}
.status-dot.connected { background: #3b82f6; box-shadow: 0 0 10px #3b82f6; }
.status-dot.busy { background: #a855f7; box-shadow: 0 0 10px #a855f7; }
.quick-switcher-icon { color: #52525b; display: flex; align-items: center; }
.tab-icon { margin-right: calc(8px * var(--ter-ui-scale)); font-size: calc(12px * var(--ter-ui-scale)); opacity: 0.5; }
.tab-item.active { color: #fafafa; background: rgba(255, 255, 255, 0.02); }
.active-bar { position: absolute; bottom: 0; left: 0; width: 100%; height: 2px; background: #3b82f6; }
.active-bar-secondary { position: absolute; bottom: 0; left: 0; width: 100%; height: 2px; background: #a855f7; }
.tab-item .btn-close { position: absolute; right: calc(8px * var(--ter-ui-scale)); background: transparent; border: none; color: #52525b; cursor: pointer; opacity: 0; transition: all 0.2s; z-index: 5; }
.tab-item .btn-close:hover { opacity: 1; color: #ef4444 !important; }
</style>
