<script setup lang="ts">
import { computed } from 'vue';
import { WIDGET_REGISTRY } from '../WidgetRegistry';

defineOptions({ inheritAttrs: false });

const props = defineProps<{
  widgetId: string;
  // Pass all common props that components might need
  widgetProps?: any;
}>();

const widgetComponent = computed(() => WIDGET_REGISTRY[props.widgetId]);

const getTitle = computed(() => {
  return props.widgetId.replace(/_/g, ' ');
});

const showHeader = computed(() => {
  // Hide header for specific structural panels to avoid ugly redundancy
  return !['SIDEBAR_PANEL', 'TERMINAL_MAIN', 'CYBER_HUD'].includes(props.widgetId);
});

// v2.14.17: Explicit Event Tunneling
const emit = defineEmits([
  'switch-tab', 'proc-context', 'terminal-context', 'run-skill', 
  'fast-access', 'explorer-context', 'resize-sftp-start',
  'change-dir', 'switch-web', 'web-context', 'open-vault-entry', 'view-changed'
]);
</script>

<template>
  <div class="tile-container">
    <header v-if="showHeader" class="tile-header">
      <span class="tile-title">{{ getTitle }}</span>
      <div class="tile-controls">
        <button class="tile-btn mini">_</button>
        <button class="tile-btn mini">×</button>
      </div>
    </header>
    <div class="tile-content">
      <component :is="widgetComponent" v-bind="widgetProps" v-on="$attrs" />
    </div>
  </div>
</template>

<style scoped>
.tile-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background: #000;
  border: 1px solid #18181b;
  overflow: hidden;
}

.tile-header {
  height: calc(24px * var(--ter-ui-scale));
  background: #050505;
  border-bottom: 1px solid #18181b;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 calc(10px * var(--ter-ui-scale));
}

.tile-title {
  font-size: calc(9px * var(--ter-ui-scale));
  color: #52525b;
  font-family: 'JetBrains Mono', monospace;
  letter-spacing: 1px;
}

.tile-controls {
  display: flex;
  gap: 5px;
}

.tile-btn {
  background: transparent;
  border: none;
  color: #3f3f46;
  cursor: pointer;
  font-size: 12px;
}
.tile-btn:hover { color: #fff; }

.tile-content {
  flex: 1;
  overflow: hidden;
  position: relative;
}
</style>
