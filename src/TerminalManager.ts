import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';

export interface TerminalInstance {
  id: string;
  term: Terminal;
  fit: FitAddon;
  webgl?: WebglAddon;
}

class TerminalManager {
  private instances: Map<string, TerminalInstance> = new Map();
  private callbacks: Map<string, (data: string) => void> = new Map();

  public setOnDataCallback(id: string, cb: (data: string) => void) {
    this.callbacks.set(id, cb);
  }

  public getOrCreate(id: string, options: any = {}): TerminalInstance {
    const existing = this.instances.get(id);
    if (existing) return existing;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', monospace",
      theme: { background: '#000', foreground: '#fafafa' },
      allowTransparency: true,
      ...options
    });

    const fit = new FitAddon();
    term.loadAddon(fit);

    // Atomic data binding
    term.onData((data) => {
      const cb = this.callbacks.get(id);
      if (cb) cb(data);
    });

    const instance: TerminalInstance = { id, term, fit };
    this.instances.set(id, instance);
    return instance;
  }

  public mount(id: string, el: HTMLElement) {
    const instance = this.instances.get(id);
    if (instance) {
      console.log(`[Manager] Mounting terminal ${id} to DOM`);
      instance.term.open(el);
      
      // Try to load WebGL for performance if not already loaded
      if (!instance.webgl) {
        try {
          const webgl = new WebglAddon();
          instance.term.loadAddon(webgl);
          instance.webgl = webgl;
        } catch (e) {
          console.warn("[Manager] WebGL addon failed to load:", e);
        }
      }
      
      // Initial fit
      instance.fit.fit();
    }
  }

  public fit(id: string) {
    const instance = this.instances.get(id);
    if (instance && instance.term.element) {
      instance.fit.fit();
    }
  }

  public focus(id: string) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.focus();
    }
  }

  public write(id: string, data: string | Uint8Array) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.write(data);
    }
  }

  public broadcast(data: string | Uint8Array) {
    this.instances.forEach((instance) => {
      instance.term.write(data);
    });
  }

  public remove(id: string) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.dispose();
      this.instances.delete(id);
      this.callbacks.delete(id);
    }
  }
}

export const terminalManager = new TerminalManager();
