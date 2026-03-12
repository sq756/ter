import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import { invoke } from '@tauri-apps/api/core';

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
  private static instance: TerminalManager;

  constructor() {
    if (TerminalManager.instance) return TerminalManager.instance;
    TerminalManager.instance = this;
  }

  public setOnDataCallback(id: string, cb: (id: string, data: string) => void) {
    this.callbacks.set(id, cb);
  }

  public getOrCreate(id: string, options: any = {}): TerminalInstance {
    const existing = this.instances.get(id);
    if (existing) return existing;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
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

  /**
   * Explicitly mount terminal to a DOM element.
   * Ensures that the terminal is correctly attached and focused.
   */
  public mount(id: string, element: HTMLElement) {
    const instance = this.getOrCreate(id);
    if (instance.term.element) {
      if (instance.term.element === element) return;
      // If already mounted elsewhere, xterm handles relocation but we clear old parent
      if (instance.term.element.parentElement) {
        instance.term.element.parentElement.innerHTML = '';
      }
    }
    element.innerHTML = '';
    instance.term.open(element);
    instance.fit.fit();
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

  public hasSelection(id: string): boolean {
    const instance = this.instances.get(id);
    return instance ? instance.term.hasSelection() : false;
  }

  public getBufferText(id: string, lines: number = 50): string {
    const instance = this.instances.get(id);
    if (!instance) return '';
    const term = instance.term;
    const buffer = term.buffer.active;
    let result = '';
    const start = Math.max(0, buffer.baseY + buffer.cursorY - lines);
    const end = buffer.baseY + buffer.cursorY;
    for (let i = start; i <= end; i++) {
      const line = buffer.getLine(i);
      if (line) result += line.translateToString() + '\n';
    }
    return result.trim();
  }

  public write(id: string, data: string | Uint8Array) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.write(data);
    }
  }

  public remove(id: string) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.dispose();
      this.instances.delete(id);
      this.callbacks.delete(id);
      // Clean up backend PTY resources
      invoke('close_pty', { tabId: id }).catch(e => {
        console.error("Failed to close backend PTY:", e);
      });
    }
  }
}

export const terminalManager = new TerminalManager();
export { WebglAddon };
