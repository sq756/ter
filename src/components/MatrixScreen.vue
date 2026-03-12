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
  const ctx = canvas.getContext('2d', { alpha: false })!; // Alpha false for performance
  
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
    columnLogs[x] = "SYSTEM_INITIALIZING...";
  }

  const draw = () => {
    // Persistent Black Background with slight trail (Dynamic trail length)
    ctx.fillStyle = 'rgba(0, 0, 0, 0.12)';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    ctx.font = `bold ${fontSize}px 'JetBrains Mono', monospace`;

    for (let i = 0; i < drops.length; i++) {
      const dropY = drops[i];
      let logLine = columnLogs[i];
      if (dropY === undefined || logLine === undefined) continue;

      const charIndex = Math.floor(dropY) % logLine.length;
      const char = logLine[charIndex] || " ";

      const yPos = dropY * fontSize;
      const brightness = Math.max(0.05, 1 - (yPos / canvas.height));
      
      // Feature: Randomly highlight actual logs with glowing white/green
      const isRealLog = logLine.startsWith("[") || logLine.includes("DEBUG");
      
      if (isRealLog && Math.random() > 0.98) {
        ctx.fillStyle = '#fff';
        ctx.shadowBlur = 12;
        ctx.shadowColor = '#0f0';
      } else {
        ctx.fillStyle = `rgba(0, 255, 70, ${brightness})`;
        ctx.shadowBlur = 0;
      }

      ctx.fillText(char, i * fontSize, yPos);

      // Dynamic Speed based on CPU usage (Turbo mode when high load)
      const speed = 0.5 + (props.cpuUsage / 100) * 3.0;
      
      if (yPos > canvas.height && Math.random() > 0.975) {
        drops[i] = 0;
        // Inject recent logs into columns
        if (props.logs.length > 0 && Math.random() > 0.4) {
          const rawLog = props.logs[Math.floor(Math.random() * props.logs.length)];
          if (rawLog) {
            // Clean up ANSI and pick a slice
            columnLogs[i] = rawLog.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '').substring(0, 60);
          } else {
            columnLogs[i] = "NODE_TRACE_NULL";
          }
        } else {
          columnLogs[i] = "NODE_TRACE_" + Math.random().toString(16).slice(2, 10).toUpperCase();
        }
      } else {
        drops[i] = dropY + speed;
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
          
          <!-- Live CyberLog Viewer -->
          <div class="live-logs-window">
             <div v-for="(log, idx) in logs.slice(-5)" :key="idx" class="log-entry">
               <span class="timestamp">[{{ new Date().toLocaleTimeString() }}]</span>
               <span class="content">{{ log.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '').substring(0, 100) }}</span>
             </div>
          </div>

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
  opacity: 0.04;
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
  background: radial-gradient(circle at center, transparent 20%, rgba(0, 0, 0, 0.7) 100%);
  pointer-events: none;
}

.security-barrier {
  text-align: center;
  border: 1px solid rgba(0, 255, 70, 0.4);
  padding: 50px 80px;
  background: rgba(0, 10, 0, 0.45);
  backdrop-filter: blur(15px) saturate(180%);
  position: relative;
  box-shadow: 
    0 0 50px rgba(0, 255, 70, 0.1),
    inset 0 0 20px rgba(0, 255, 70, 0.05);
  border-radius: 4px;
}

.live-logs-window {
  margin-top: 30px;
  text-align: left;
  font-family: 'JetBrains Mono', monospace;
  background: rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(0, 255, 70, 0.2);
  padding: 15px;
  border-radius: 4px;
  height: 110px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 6px;
  box-shadow: inset 0 0 10px rgba(0, 0, 0, 0.8);
}

.log-entry {
  font-size: 11px;
  color: rgba(0, 255, 70, 0.7);
  white-space: nowrap;
  animation: log-slide 0.3s ease-out;
}

.log-entry .timestamp {
  color: #333;
  margin-right: 10px;
  font-size: 10px;
}

.log-entry .content {
  color: #0f0;
  text-shadow: 0 0 5px rgba(0, 255, 70, 0.5);
}

@keyframes log-slide {
  from { opacity: 0; transform: translateY(5px); }
  to { opacity: 1; transform: translateY(0); }
}

.glitch-wrapper {
  position: relative;
  display: inline-block;
}

.glitch-text {
  font-size: 42px;
  font-weight: 800;
  color: #fff;
  letter-spacing: 16px;
  font-family: 'JetBrains Mono', monospace;
  text-shadow: 0 0 15px rgba(0, 255, 70, 0.9);
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
  font-size: 11px;
  margin-top: 15px;
  letter-spacing: 6px;
  font-family: 'JetBrains Mono', monospace;
  opacity: 0.8;
  text-transform: uppercase;
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
  bottom: 50px;
  right: 50px;
  text-align: right;
  font-family: 'JetBrains Mono', monospace;
  border-right: 2px solid #0f0;
  padding-right: 20px;
  background: rgba(0, 0, 0, 0.3);
  padding: 15px;
  backdrop-filter: blur(5px);
}

.monitor-item {
  margin-bottom: 5px;
}

.monitor-item .label {
  font-size: 10px;
  color: #666;
  margin-right: 12px;
}

.monitor-item .value {
  font-size: 14px;
  color: #fff;
  text-shadow: 0 0 10px #0f0;
  font-weight: bold;
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
