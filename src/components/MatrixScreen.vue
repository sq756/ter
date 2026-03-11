<script setup lang="ts">
import { ref, watch, nextTick, onUnmounted } from 'vue';

const props = defineProps<{
  isLocked: boolean;
  logs: string[];
}>();

const matrixCanvas = ref<HTMLCanvasElement | null>(null);
let matrixInterval: any = null;

const startMatrix = () => {
  const canvas = matrixCanvas.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d')!;
  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;

  const fontSize = 14;
  const columns = canvas.width / fontSize;
  const drops: number[] = [];
  for (let x = 0; columns > x; x++) drops[x] = 1;

  const draw = () => {
    ctx.fillStyle = 'rgba(0, 0, 0, 0.05)';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = '#0F0';
    ctx.font = fontSize + 'px monospace';

    for (let i = 0; drops.length > i; i++) {
      const charSource = props.logs.length > 0 
        ? props.logs[props.logs.length - 1] 
        : "TER-HACKER-ACTIVE-01";
      
      if (!charSource) continue;
      const text = charSource[Math.floor(Math.random() * charSource.length)] || "X";
      
      const dropY = drops[i] ?? 0;
      ctx.fillText(text, i * fontSize, dropY * fontSize);
      if (dropY * fontSize > canvas.height && Math.random() > 0.975) {
        drops[i] = 0;
      } else {
        drops[i] = dropY + 1;
      }
    }
  };
  matrixInterval = setInterval(draw, 33);
};

watch(() => props.isLocked, (val) => {
  if (val) nextTick(() => startMatrix());
  else if (matrixInterval) clearInterval(matrixInterval);
});

onUnmounted(() => {
  if (matrixInterval) clearInterval(matrixInterval);
});
</script>

<template>
  <div v-if="isLocked" class="lock-screen">
    <canvas ref="matrixCanvas"></canvas>
    <div class="lock-overlay">
      <h1>SECURE SESSION LOCKED</h1>
      <p>Press Alt+L or Click to Unlock</p>
      <div class="log-glitch">{{ logs[logs.length-1] }}</div>
    </div>
  </div>
</template>

<style scoped>
.lock-screen { position: fixed; inset: 0; z-index: 20000; background: #000; cursor: pointer; }
canvas { display: block; }
.lock-overlay { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; background: rgba(0,0,0,0.7); text-align: center; pointer-events: none; }
.lock-overlay h1 { font-size: 24px; color: #0F0; letter-spacing: 4px; text-shadow: 0 0 10px #0F0; font-family: monospace; }
.lock-overlay p { color: #52525b; font-size: 12px; margin-top: 10px; }
.log-glitch { font-family: monospace; font-size: 10px; color: #0F0; opacity: 0.5; margin-top: 40px; max-width: 80%; overflow: hidden; white-space: nowrap; }
</style>
