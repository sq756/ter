import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';

export interface TerminalInstance {
  id: string;
  term: Terminal;
  fit: FitAddon;
}

/**
 * Registry Pattern: TerminalManager only manages terminal instances in memory.
 * DOM operations are handled by the TerminalView component.
 */
class TerminalManager {
  private instances: Map<string, TerminalInstance> = new Map();
  private callbacks: Map<string, (id: string, data: string) => void> = new Map();

  public setOnDataCallback(id: string, cb: (id: string, data: string) => void) {
    this.callbacks.set(id, cb);
  }

  public getOrCreate(id: string, options: any = {}): TerminalInstance {
    const existing = this.instances.get(id);
    if (existing) return existing;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', monospace",
      theme: { background: '#09090b', foreground: '#d4d4d8' },
      allowTransparency: false,
      ...options
    });

    const fit = new FitAddon();
    term.loadAddon(fit);

    // Atomic data binding
    term.onData((data) => {
      const cb = this.callbacks.get(id);
      if (cb) cb(id, data);
    });

    const instance: TerminalInstance = { id, term, fit };
    this.instances.set(id, instance);
    return instance;
  }

  public fitAll() {
    this.instances.forEach((instance) => {
      if (instance.term.element) {
        instance.fit.fit();
      }
    });
  }

  public focus(id: string) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.focus();
    }
  }

  public getSelection(id: string): string {
    const instance = this.instances.get(id);
    return instance ? instance.term.getSelection() : '';
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
export { WebglAddon };
