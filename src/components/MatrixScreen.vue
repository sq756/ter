<script setup lang="ts">
import { ref, watch, nextTick, onUnmounted, onMounted } from 'vue';

const props = defineProps<{
  isLocked: boolean;
  logs: string[];
  cpuUsage: number;
}>();

const matrixCanvas = ref<HTMLCanvasElement | null>(null);
let matrixInterval: any = null;
const systemTime = ref(new Date().toLocaleTimeString());
const bitrate = ref((Math.random() * 1000 + 500).toFixed(2));
const isTyping = ref(false);

onMounted(() => {
  setInterval(() => {
    systemTime.value = new Date().toLocaleTimeString();
    bitrate.value = (Math.random() * 1000 + 500).toFixed(2);
  }, 1000);
});

const startMatrix = () => {
  const canvas = matrixCanvas.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d', { alpha: false })!; 
  
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
      
      const isRealLog = logLine.startsWith("[") || logLine.includes("DEBUG");
      
      if (isRealLog && Math.random() > 0.98) {
        ctx.fillStyle = '#fff';
        ctx.shadowBlur = 12;
        ctx.shadowColor = '#22c55e';
      } else {
        ctx.fillStyle = `rgba(34, 197, 94, ${brightness})`;
        ctx.shadowBlur = 0;
      }

      ctx.fillText(char, i * fontSize, yPos);

      // v2.2.11: Speed up by 5x if typing (simulating crack)
      const baseSpeed = 0.5 + (props.cpuUsage / 100) * 3.0;
      const speed = isTyping.value ? baseSpeed * 5 : baseSpeed;
      
      if (yPos > canvas.height && Math.random() > 0.975) {
        drops[i] = 0;
        if (props.logs.length > 0 && Math.random() > 0.4) {
          const rawLog = props.logs[Math.floor(Math.random() * props.logs.length)];
          if (rawLog) {
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

const handleKeyDown = (e: KeyboardEvent) => {
  isTyping.value = true;
  setTimeout(() => isTyping.value = false, 100);
  if (e.key === 'Enter' || e.key === 'Escape') {
    emit('unlock');
  }
};

watch(() => props.isLocked, (val) => {
  if (val) {
    nextTick(() => startMatrix());
    window.addEventListener('resize', handleResize);
    window.addEventListener('keydown', handleKeyDown);
  } else {
    if (matrixInterval) clearInterval(matrixInterval);
    window.removeEventListener('resize', handleResize);
    window.removeEventListener('keydown', handleKeyDown);
  }
});

const handleResize = () => {
  if (props.isLocked) {
    const canvas = matrixCanvas.value;
    if (canvas) {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    }
  }
};

const emit = defineEmits(['unlock']);

onUnmounted(() => {
  if (matrixInterval) clearInterval(matrixInterval);
  window.removeEventListener('resize', handleResize);
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<template>
  <Transition name="fade">
    <div v-if="isLocked" class="matrix-container">
      <canvas ref="matrixCanvas"></canvas>
      
      <div class="blur-overlay"></div>
      <div class="scanlines"></div>

      <div class="matrix-overlay">
        <div class="security-barrier">
          <div class="glitch-wrapper">
            <div class="glitch-text" data-text="AUTHENTICATION REQUIRED">AUTHENTICATION REQUIRED</div>
          </div>
          <div class="barrier-sub">LEVEL 7 CLEARANCE DETECTED // ENCRYPTING TRACE</div>
          
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

        <div class="system-stats-corner top-left">
          <div class="stat-item">SYSTEM_CLOCK: <span class="highlight">{{ systemTime }}</span></div>
          <div class="stat-item">ENCRYPTION: <span class="highlight">{{ bitrate }} bits/s</span></div>
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
}

.blur-overlay {
  position: absolute;
  inset: 0;
  backdrop-filter: blur(15px);
  z-index: 5;
  pointer-events: none;
}

canvas {
  display: block;
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

.matrix-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  z-index: 20;
  background: radial-gradient(circle at center, transparent 20%, rgba(0, 0, 0, 0.7) 100%);
}

.security-barrier {
  text-align: center;
  border: 1px solid #22c55e;
  padding: 50px 80px;
  background: rgba(0, 10, 0, 0.4);
  position: relative;
  box-shadow: 0 0 30px rgba(34, 197, 94, 0.3);
  border-radius: 8px;
  animation: breathing-glow 4s infinite ease-in-out;
}

@keyframes breathing-glow {
  0%, 100% { box-shadow: 0 0 20px rgba(34, 197, 94, 0.2); }
  50% { box-shadow: 0 0 40px rgba(34, 197, 94, 0.5); }
}

.live-logs-window {
  margin-top: 30px;
  text-align: left;
  font-family: 'JetBrains Mono', monospace;
  background: rgba(0, 0, 0, 0.8);
  border: 1px solid rgba(34, 197, 94, 0.2);
  padding: 15px;
  border-radius: 4px;
  height: 110px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.log-entry {
  font-size: 11px;
  color: #22c55e;
  white-space: nowrap;
}

.log-entry .timestamp {
  color: #166534;
  margin-right: 10px;
  font-size: 10px;
}

.log-entry .content {
  color: #22c55e;
  text-shadow: 0 0 5px rgba(34, 197, 94, 0.5);
}

.glitch-text {
  font-size: 42px;
  font-weight: 800;
  color: #22c55e;
  letter-spacing: 16px;
  font-family: 'JetBrains Mono', monospace;
  text-shadow: 0 0 15px rgba(34, 197, 94, 0.8);
}

.barrier-sub {
  color: #22c55e;
  font-size: 11px;
  margin-top: 15px;
  letter-spacing: 6px;
  font-family: 'JetBrains Mono', monospace;
  opacity: 0.8;
  text-transform: uppercase;
}

.security-lines .line {
  height: 100%;
  width: 40%;
  background: #22c55e;
  box-shadow: 0 0 10px #22c55e;
}

.pulse-text {
  color: #166534;
  font-size: 14px;
  text-transform: uppercase;
  letter-spacing: 6px;
  animation: pulse 4s infinite;
  font-family: monospace;
}

@keyframes pulse {
  0%, 100% { opacity: 0.2; color: #166534; }
  50% { opacity: 0.8; color: #22c55e; text-shadow: 0 0 8px #22c55e; }
}

.system-stats-corner {
  position: absolute;
  top: 40px;
  left: 40px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: #166534;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.stat-item .highlight {
  color: #22c55e;
  text-shadow: 0 0 5px #22c55e;
}

.status-monitor {
  position: absolute;
  bottom: 50px;
  right: 50px;
  text-align: right;
  font-family: 'JetBrains Mono', monospace;
  border-right: 2px solid #22c55e;
  padding: 15px 20px;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(5px);
}

.monitor-item .label {
  font-size: 10px;
  color: #166534;
}

.monitor-item .value {
  font-size: 14px;
  color: #22c55e;
  text-shadow: 0 0 10px #22c55e;
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
