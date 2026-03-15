<script lang="ts">
export default {
  name: 'GridEngine'
}
</script>

<script setup lang="ts">
import { computed } from 'vue';
import type { LayoutNode } from '../WidgetRegistry';
import TileContainer from './TileContainer.vue';

const props = defineProps<{
  node: LayoutNode;
  sharedProps?: any;
}>();

const containerStyle = computed(() => {
  if (props.node.type === 'widget') return { flex: 1 };
  
  return {
    display: 'flex',
    flexDirection: props.node.type === 'split-horizontal' ? 'row' : 'column',
    flex: 1,
    height: '100%',
    width: '100%',
    gap: '2px'
  };
});

const firstStyle = computed(() => ({
  flex: props.node.ratio || 0.5,
  overflow: 'hidden'
}));

const secondStyle = computed(() => ({
  flex: 1 - (props.node.ratio || 0.5),
  overflow: 'hidden'
}));
</script>

<template>
  <div v-if="node.type === 'widget'" class="grid-widget-box">
    <TileContainer :widgetId="node.id!" :widgetProps="sharedProps" v-on="$attrs" />
  </div>
  
  <div v-else :style="containerStyle" class="grid-split-box">
    <div :style="firstStyle">
      <GridEngine :node="node.left || node.top!" :sharedProps="sharedProps" v-on="$attrs" />
    </div>
    <div :style="secondStyle">
      <GridEngine :node="node.right || node.bottom!" :sharedProps="sharedProps" v-on="$attrs" />
    </div>
  </div>
</template>

<style scoped>
.grid-widget-box, .grid-split-box {
  height: 100%;
  width: 100%;
}
</style>
