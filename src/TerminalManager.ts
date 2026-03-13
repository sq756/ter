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
    
    // Global for debugging
    if (typeof window !== 'undefined') {
      (window as any).terminalManager = this;
    }
  }

  public setOnDataCallback(id: string, cb: (id: string, data: string) => void) {
    this.callbacks.set(id, cb);
  }

  public getOrCreate(id: string, options: any = {}): TerminalInstance {
    const existing = this.instances.get(id);
    if (existing) return existing;

    console.log(`[TerminalManager] Creating terminal instance: ${id}`);
    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Ubuntu Mono', 'Fira Code', monospace",
      theme: { background: '#09090b', foreground: '#d4d4d8' },
      allowTransparency: false,
      ...options
    });

    const fit = new FitAddon();
    term.loadAddon(fit);

    // Atomic data binding
    term.onData((data) => {
      const cb = this.callbacks.get(id);
      if (cb) {
        cb(id, data);
      } else {
        console.warn(`[TerminalManager] No data callback for terminal ${id}`);
      }
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
    console.log(`[TerminalManager] Mounting terminal ${id} to element`, element);
    const instance = this.getOrCreate(id);
    
    if (instance.term.element) {
      if (instance.term.element === element) {
        console.log(`[TerminalManager] Terminal ${id} already mounted to this element`);
        return;
      }
      console.log(`[TerminalManager] Terminal ${id} relocation from`, instance.term.element);
      if (instance.term.element.parentElement) {
        instance.term.element.parentElement.innerHTML = '';
      }
    }
    
    element.innerHTML = '';
    try {
      instance.term.open(element);
      // Wait for next frame to ensure DOM is ready for measurement
      requestAnimationFrame(() => {
        if (element.offsetWidth > 0) {
          instance.fit.fit();
          console.log(`[TerminalManager] Initial fit for ${id}: ${instance.term.cols}x${instance.term.rows}`);
        } else {
          console.warn(`[TerminalManager] Element for ${id} has 0 width during mount`);
        }
      });
    } catch (e) {
      console.error(`[TerminalManager] Failed to open terminal ${id}:`, e);
    }
  }

  public fitAll() {
    this.instances.forEach((instance) => {
      if (instance.term.element && instance.term.element.offsetWidth > 0) {
        instance.fit.fit();
      }
    });
  }

  public focus(id: string) {
    const instance = this.instances.get(id);
    if (instance) {
      instance.term.focus();
    } else {
      console.warn(`[TerminalManager] Cannot focus, instance ${id} not found`);
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
    } else {
      // Very frequent logging might be noisy, but useful for diagnosis
      // console.warn(`[TerminalManager] Cannot write, instance ${id} not found`);
    }
  }

  public remove(id: string) {
    const instance = this.instances.get(id);
    if (instance) {
      console.log(`[TerminalManager] Removing terminal ${id}`);
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
