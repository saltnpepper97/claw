import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

export type Keybinds = {
  historyUp: string;
  historyDown: string;
  useEntry: string;
  removeEntry: string;
  deleteAll: string;
};

// Default keybinds
export const keybinds = writable<Keybinds>({
  historyUp: 'ArrowUp',
  historyDown: 'ArrowDown',
  useEntry: 'Enter',
  removeEntry: 'x',
  deleteAll: '' // empty by default
});

// Normalize config key strings
function normalizeKey(key: string) {
  const k = key.trim();
  switch (k.toLowerCase()) {
    case 'up': return 'ArrowUp';
    case 'down': return 'ArrowDown';
    case 'left': return 'ArrowLeft';
    case 'right': return 'ArrowRight';
    default: return k; // keep letters, Delete, Return, Shift+X, etc.
  }
}

// Set keybinds from config
export function setKeybindsFromConfig(config: {
  keybinds: {
    up: string;
    down: string;
    select: string;
    delete: string;
    delete_all: string;
  };
}) {
  keybinds.set({
    historyUp: normalizeKey(config.keybinds.up || 'ArrowUp'),
    historyDown: normalizeKey(config.keybinds.down || 'ArrowDown'),
    useEntry: config.keybinds.select || 'Enter',
    removeEntry: config.keybinds.delete || 'x',
    deleteAll: config.keybinds.delete_all || ''
  });
}

// Check if a KeyboardEvent matches a keybind (supports Shift, Ctrl, Alt)
export function matchKeybind(event: KeyboardEvent, keybind: string) {
  const parts = keybind.split('+').map(p => p.trim().toLowerCase());
  const baseKey = parts.pop()!;
  const shiftRequired = parts.includes('shift');
  const ctrlRequired = parts.includes('ctrl');
  const altRequired = parts.includes('alt');

  // Normalize event.key for comparison
  let eventKey = event.key;
  if (/Arrow(up|down|left|right)/i.test(eventKey)) {
    // Keep Arrow keys as-is
  } else {
    eventKey = eventKey.toLowerCase();
  }

  return (
    eventKey === baseKey &&
    event.shiftKey === shiftRequired &&
    event.ctrlKey === ctrlRequired &&
    event.altKey === altRequired
  );
}

// Load keybinds from Rust backend
export async function loadKeybindsFromBackend() {
  try {
    const cfg = await invoke('get_claw_config');
    setKeybindsFromConfig(cfg);
    console.log('Loaded keybinds from backend:', get(keybinds));
  } catch (err) {
    console.error('Failed to load keybinds from backend:', err);
  }
}
