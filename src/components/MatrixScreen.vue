<script setup lang="ts">
import { ref, watch, nextTick, onUnmounted } from 'vue';

const props = defineProps<{
  isLocked: boolean;
  logs: string[];
  cpuUsage: number;
}>();

const matrixCanvas = ref<HTMLCanvasElement | null>(null);
let matrixInterval: any = null;

const startMatrix = () => {
  const canvas = matrixCanvas.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d')!;
  
  const setCanvasSize = () => {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  };
  setCanvasSize();

  const fontSize = 14;
  const columns = Math.floor(canvas.width / fontSize);
  
  const drops: number[] = [];
  const columnLogs: string[] = [];
  
  for (let x = 0; x < columns; x++) {
    drops[x] = Math.random() * -100;
    columnLogs[x] = props.logs[Math.floor(Math.random() * props.logs.length)] || "SYSTEM.CORE.AUDIT.INIT...";
  }

  const draw = () => {
    // Persistent Black Background with slight trail
    ctx.fillStyle = 'rgba(0, 0, 0, 0.08)';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    ctx.font = `bold ${fontSize}px 'JetBrains Mono', monospace`;

    for (let i = 0; i < drops.length; i++) {
      const logLine = columnLogs[i];
      const charIndex = Math.floor(drops[i]) % logLine.length;
      const char = logLine[charIndex] || " ";

      // Brightness: Darker at bottom, bright at the "head" of the drop
      const yPos = drops[i] * fontSize;
      const brightness = Math.max(0.05, 1 - (yPos / canvas.height));
      
      // Leading character is brighter/white
      if (Math.random() > 0.98) {
        ctx.fillStyle = '#fff';
      } else {
        ctx.fillStyle = `rgba(0, 255, 70, ${brightness})`;
      }

      ctx.fillText(char, i * fontSize, yPos);

      // Dynamic Speed based on CPU usage (0.5 to 3.0 range)
      const speed = 0.8 + (props.cpuUsage / 100) * 2.2;
      
      if (yPos > canvas.height && Math.random() > 0.975) {
        drops[i] = 0;
        columnLogs[i] = props.logs[Math.floor(Math.random() * props.logs.length)] || "RE-ENCRYPTING.DATA.STREAM";
      } else {
        drops[i] += speed;
      }
    }
  };

  if (matrixInterval) clearInterval(matrixInterval);
  matrixInterval = setInterval(draw, 33);
};

const handleResize = () => {
  if (props.isLocked) {
    const canvas = matrixCanvas.value;
    if (canvas) {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    }
  }
};

watch(() => props.isLocked, (val) => {
  if (val) {
    nextTick(() => startMatrix());
    window.addEventListener('resize', handleResize);
    window.addEventListener('keydown', handleUnlock);
  } else {
    if (matrixInterval) clearInterval(matrixInterval);
    window.removeEventListener('resize', handleResize);
    window.removeEventListener('keydown', handleUnlock);
  }
});

const emit = defineEmits(['unlock']);
const handleUnlock = () => emit('unlock');

onUnmounted(() => {
  if (matrixInterval) clearInterval(matrixInterval);
  window.removeEventListener('resize', handleResize);
  window.removeEventListener('keydown', handleUnlock);
});
</script>

<template>
  <Transition name="fade">
    <div v-if="isLocked" class="matrix-container" @click="handleUnlock">
      <canvas ref="matrixCanvas"></canvas>
      
      <div class="scanlines"></div>
      <div class="noise"></div>

      <div class="matrix-overlay">
        <div class="security-barrier">
          <div class="glitch-wrapper">
            <div class="glitch-text" data-text="AUTHENTICATION REQUIRED">AUTHENTICATION REQUIRED</div>
          </div>
          <div class="barrier-sub">LEVEL 7 CLEARANCE DETECTED // ENCRYPTING TRACE</div>
          <div class="security-lines">
            <div class="line"></div>
            <div class="line"></div>
          </div>
        </div>
        
        <div class="decrypt-prompt">
          <span class="pulse-text">PRESS ANY KEY TO DECRYPT</span>
        </div>

        <div class="status-monitor">
          <div class="monitor-item">
            <span class="label">CPU_LOAD:</span>
            <span class="value">{{ cpuUsage.toFixed(1) }}%</span>
          </div>
          <div class="monitor-item">
            <span class="label">LOG_DENSITY:</span>
            <span class="value">{{ logs.length }}_NODES</span>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.matrix-container {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 999999;
  background: #000;
  overflow: hidden;
  cursor: none;
}

canvas {
  display: block;
  filter: blur(0.4px);
}

.scanlines {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    to bottom,
    transparent 50%,
    rgba(0, 0, 0, 0.4) 50%
  );
  background-size: 100% 4px;
  pointer-events: none;
  z-index: 10;
  animation: scanline-scroll 10s linear infinite;
}

@keyframes scanline-scroll {
  from { background-position: 0 0; }
  to { background-position: 0 100%; }
}

.noise {
  position: absolute;
  inset: 0;
  background: url('data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADIAAAAyBAMAAADsEZWCAAAAGFBMVEUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAs697FAAAACHRSTlMA7v7+/v7+/v7+U88qAAAANUlEQVQ4y2NgQAX8DIyMTIwMDEwMDIyMTIyMDEwMDIyMTIyMDEwMDIyMTIyMDEwMDIyMTIyMTAwAtSADf99S99EAAAAASUVORK5CYII=');
  opacity: 0.05;
  pointer-events: none;
  z-index: 11;
}

.matrix-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  z-index: 20;
  background: radial-gradient(circle at center, transparent 30%, rgba(0, 0, 0, 0.6) 100%);
  pointer-events: none;
}

.security-barrier {
  text-align: center;
  border: 1px solid rgba(0, 255, 70, 0.3);
  padding: 40px 80px;
  background: rgba(0, 10, 0, 0.7);
  backdrop-filter: blur(10px);
  position: relative;
  box-shadow: 0 0 30px rgba(0, 255, 70, 0.1);
}

.glitch-wrapper {
  position: relative;
  display: inline-block;
}

.glitch-text {
  font-size: 48px;
  font-weight: 900;
  color: #fff;
  letter-spacing: 12px;
  font-family: 'JetBrains Mono', monospace;
  text-shadow: 0 0 10px rgba(0, 255, 70, 0.8);
}

.glitch-text::before,
.glitch-text::after {
  content: attr(data-text);
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: transparent;
}

.glitch-text::before {
  left: 2px;
  text-shadow: -2px 0 #ff003c;
  clip: rect(44px, 450px, 56px, 0);
  animation: glitch-anim 5s infinite linear alternate-reverse;
}

.glitch-text::after {
  left: -2px;
  text-shadow: -2px 0 #00fff9;
  clip: rect(0, 450px, 20px, 0);
  animation: glitch-anim2 3s infinite linear alternate-reverse;
}

@keyframes glitch-anim {
  0% { clip: rect(10px, 9999px, 20px, 0); }
  20% { clip: rect(40px, 9999px, 50px, 0); }
  40% { clip: rect(80px, 9999px, 90px, 0); }
  60% { clip: rect(20px, 9999px, 30px, 0); }
  80% { clip: rect(60px, 9999px, 70px, 0); }
  100% { clip: rect(90px, 9999px, 100px, 0); }
}

@keyframes glitch-anim2 {
  0% { clip: rect(85px, 9999px, 95px, 0); }
  30% { clip: rect(5px, 9999px, 15px, 0); }
  60% { clip: rect(45px, 9999px, 55px, 0); }
  90% { clip: rect(25px, 9999px, 35px, 0); }
}

.barrier-sub {
  color: #0f0;
  font-size: 12px;
  margin-top: 15px;
  letter-spacing: 4px;
  font-family: monospace;
  opacity: 0.6;
}

.security-lines {
  position: absolute;
  bottom: -2px;
  left: 0;
  width: 100%;
  height: 2px;
  display: flex;
  justify-content: space-between;
}

.security-lines .line {
  height: 100%;
  width: 40%;
  background: #0f0;
  box-shadow: 0 0 10px #0f0;
}

.decrypt-prompt {
  margin-top: 60px;
}

.pulse-text {
  color: #333;
  font-size: 14px;
  text-transform: uppercase;
  letter-spacing: 6px;
  animation: pulse 4s infinite;
  font-family: monospace;
}

@keyframes pulse {
  0%, 100% { opacity: 0.2; color: #333; }
  50% { opacity: 0.8; color: #0f0; text-shadow: 0 0 8px #0f0; }
}

.status-monitor {
  position: absolute;
  bottom: 40px;
  right: 40px;
  text-align: right;
  font-family: 'JetBrains Mono', monospace;
  border-right: 1px solid rgba(0, 255, 70, 0.4);
  padding-right: 15px;
}

.monitor-item {
  margin-bottom: 5px;
}

.monitor-item .label {
  font-size: 10px;
  color: #666;
  margin-right: 8px;
}

.monitor-item .value {
  font-size: 12px;
  color: #0f0;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.8s cubic-bezier(0.4, 0, 0.2, 1);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>

