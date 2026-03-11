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
  private onDataCallback: ((id: string, data: string) => void) | null = null;

  public setOnDataCallback(cb: (id: string, data: string) => void) {
    this.onDataCallback = cb;
  }

  public getOrCreate(id: string, options: any = {}): TerminalInstance {
    if (this.instances.has(id)) {
      return this.instances.get(id)!;
    }

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

    term.onData((data) => {
      if (this.onDataCallback) {
        this.onDataCallback(id, data);
      }
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
    }
  }
}

export const terminalManager = new TerminalManager();
