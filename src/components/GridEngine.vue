<script lang="ts">
export default {
  name: 'GridEngine'
}
</script>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue';
import type { LayoutNode } from '../WidgetRegistry';
import TileContainer from './TileContainer.vue';

defineOptions({ inheritAttrs: false });

const props = defineProps<{
  node: LayoutNode;
  sharedProps?: any;
}>();

const emit = defineEmits(['update-ratio']);

const isResizing = ref(false);
const containerRef = ref<HTMLElement | null>(null);

const containerStyle = computed(() => {
  if (props.node.type === 'widget') return { flex: 1 };
  
  return {
    display: 'flex',
    flexDirection: props.node.type === 'split-horizontal' ? 'row' : 'column',
    flex: 1,
    height: '100%',
    width: '100%'
  };
});

const firstStyle = computed(() => {
  if (props.node.left?.type === 'widget' && props.node.left?.id === 'SIDEBAR_PANEL' && props.sharedProps?.isSidebarOpen === false) {
    return { display: 'none' };
  }
  if (props.node.right?.type === 'widget' && props.node.right?.id === 'SIDEBAR_PANEL' && props.sharedProps?.isSidebarOpen === false) {
    return { flex: 1, overflow: 'hidden' };
  }
  return { flex: props.node.ratio || 0.5, overflow: 'hidden' };
});

const secondStyle = computed(() => {
  if (props.node.right?.type === 'widget' && props.node.right?.id === 'SIDEBAR_PANEL' && props.sharedProps?.isSidebarOpen === false) {
    return { display: 'none' };
  }
  if (props.node.left?.type === 'widget' && props.node.left?.id === 'SIDEBAR_PANEL' && props.sharedProps?.isSidebarOpen === false) {
    return { flex: 1, overflow: 'hidden' };
  }
  return { flex: 1 - (props.node.ratio || 0.5), overflow: 'hidden' };
});

const showResizer = computed(() => {
  const leftHidden = props.node.left?.type === 'widget' && props.node.left?.id === 'SIDEBAR_PANEL' && props.sharedProps?.isSidebarOpen === false;
  const rightHidden = props.node.right?.type === 'widget' && props.node.right?.id === 'SIDEBAR_PANEL' && props.sharedProps?.isSidebarOpen === false;
  return !leftHidden && !rightHidden;
});

const startResize = (e: MouseEvent) => {
  isResizing.value = true;
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResize);
};

const handleMouseMove = (e: MouseEvent) => {
  if (!isResizing.value || !containerRef.value) return;
  const rect = containerRef.value.getBoundingClientRect();
  let newRatio = 0.5;
  if (props.node.type === 'split-horizontal') {
    newRatio = (e.clientX - rect.left) / rect.width;
  } else {
    newRatio = (e.clientY - rect.top) / rect.height;
  }
  newRatio = Math.max(0.1, Math.min(0.9, newRatio));
  props.node.ratio = newRatio;
};

const stopResize = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  localStorage.setItem('ter_layout', JSON.stringify(props.node)); // Simplified persist
};

onUnmounted(() => {
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
});
</script>

<template>
  <div v-if="node.type === 'widget'" class="grid-widget-box">
    <TileContainer :widgetId="node.id!" :widgetProps="sharedProps" v-on="$attrs" />
  </div>
  
  <div v-else :style="containerStyle" class="grid-split-box" ref="containerRef">
    <div :style="firstStyle" class="pane-content">
      <GridEngine :node="node.left || node.top!" :sharedProps="sharedProps" v-on="$attrs" />
    </div>
    
    <div v-if="showResizer" class="grid-resizer" :class="node.type" @mousedown="startResize" :data-resizing="isResizing"></div>
    
    <div :style="secondStyle" class="pane-content">
      <GridEngine :node="node.right || node.bottom!" :sharedProps="sharedProps" v-on="$attrs" />
    </div>
  </div>
</template>

<style scoped>
.grid-widget-box, .grid-split-box {
  height: 100%;
  width: 100%;
}
.pane-content {
  height: 100%;
  width: 100%;
  position: relative;
}

.grid-resizer {
  background: #18181b;
  z-index: 10;
  transition: background 0.2s;
  flex: 0 0 auto;
}

.grid-resizer.split-horizontal {
  width: 4px;
  cursor: col-resize;
  border-left: 1px solid #27272a;
  border-right: 1px solid #27272a;
}
.grid-resizer.split-horizontal:hover, .grid-resizer.split-horizontal[data-resizing="true"] {
  background: #22c55e;
}

.grid-resizer.split-vertical {
  height: 4px;
  cursor: row-resize;
  border-top: 1px solid #27272a;
  border-bottom: 1px solid #27272a;
}
.grid-resizer.split-vertical:hover, .grid-resizer.split-vertical[data-resizing="true"] {
  background: #22c55e;
}
</style>
