import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebglAddon } from '@xterm/addon-webgl';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface TerminalInstance {
  id: string;
  term: Terminal;
  fit: FitAddon;
  unlisten?: UnlistenFn;
}

/**
 * Registry Pattern: TerminalManager only manages terminal instances in memory.
 * DOM operations are handled by the TerminalView component.
 */
class TerminalManager {
  public instances: Map<string, TerminalInstance> = new Map();
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

  public async getOrCreate(id: string, options: any = {}): Promise<TerminalInstance> {
    const existing = this.instances.get(id);
    if (existing) return existing;

    console.log(`[TerminalManager] Creating terminal instance: ${id}`);
    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Ubuntu Mono', 'Fira Code', monospace",
      theme: { background: '#000000', foreground: '#d4d4d8' },
      allowTransparency: false,
      scrollback: 2000, 
      wheelScrollSensitivity: 1,
      // v2.16.0: Stop ANSI DA leak (^[[?1;2c)
      // Disabling device attributes response prevents the terminal from 
      // automatically replying to \x1b[c which can pollute the PTY buffer.
      screenReaderMode: false,
      ...options
    });

    const fit = new FitAddon();
    term.loadAddon(fit);

    // v2.15.40: Re-enabling Canvas for standard rendering
    // WebGL remains disabled for multi-pane stability
    
    // Atomic data binding
    term.onData((data) => {
      const cb = this.callbacks.get(id);
      if (cb) cb(id, data);
    });

    // v2.15.35: Global Listener Registration
    if (!this.isGlobalListenerActive) {
      this.setupGlobalPtyListener();
    }

    const instance: TerminalInstance = { id, term, fit };
    this.instances.set(id, instance);
    return instance;
  }

  private isGlobalListenerActive = false;
  private async setupGlobalPtyListener() {
    this.isGlobalListenerActive = true;
    console.log("[TerminalManager] Activating Global PTY Dispatcher");
    await listen('pty-data', (event: any) => {
      const payload = event.payload as any;
      const id = payload.id;
      const data = payload.data;
      
      const instance = this.instances.get(id);
      if (instance) {
        if (Array.isArray(data)) {
          instance.term.write(new Uint8Array(data));
        } else {
          instance.term.write(data);
        }
      }
    });
  }

  /**
   * Explicitly mount terminal to a DOM element.
   */
  public async mount(id: string, element: HTMLElement) {
    const instance = await this.getOrCreate(id);
    
    // v2.16.0: Intelligent Mount Guard (Prevent Black Screen)
    if (instance.term.element) {
      if (instance.term.element.parentElement === element) {
        // Already mounted to the correct parent, just refresh
        instance.term.refresh(0, instance.term.rows - 1);
        instance.fit.fit();
        return;
      }
      // Detach from old parent before mounting to new one
      try {
        const oldParent = instance.term.element.parentElement;
        if (oldParent) {
          oldParent.innerHTML = '';
        }
      } catch (e) {}
    }
    
    try {
      element.innerHTML = ''; // Only clear the NEW parent
      instance.term.open(element);
      
      if (instance.term.element) {
        instance.term.element.onmousedown = () => {
          window.dispatchEvent(new CustomEvent('close-all-menus'));
        };
      }

      setTimeout(() => {
        if (element.offsetWidth > 0) {
          instance.fit.fit();
          instance.term.refresh(0, instance.term.rows - 1);
          instance.term.focus();
        }
      }, 50);
    } catch (e) {
      console.error(`[TerminalManager] Mount Fail ${id}:`, e);
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
