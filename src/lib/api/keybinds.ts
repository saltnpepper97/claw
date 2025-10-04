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
    if (!key || key.trim() === '') return '';
    
    const k = key.trim().toLowerCase();
    switch (k) {
        case 'up': return 'ArrowUp';
        case 'down': return 'ArrowDown';
        case 'return': return 'Enter';
        case 'enter': return 'Enter';
        case 'delete': return 'Delete';
        default: {
            // Handle modifier combinations like "Shift+x"
            if (key.includes('+')) {
                const parts = key.split('+').map(p => p.trim());
                const lastPart = parts[parts.length - 1];
                // Capitalize modifier names, keep key as lowercase
                const modifiers = parts.slice(0, -1).map(m => 
                    m.charAt(0).toUpperCase() + m.slice(1).toLowerCase()
                );
                return [...modifiers, lastPart.toLowerCase()].join('+');
            }
            return key.trim().toLowerCase();
        }
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
        useEntry: normalizeKey(config.keybinds.select || 'Enter'),
        removeEntry: normalizeKey(config.keybinds.delete || 'x'),
        deleteAll: normalizeKey(config.keybinds.delete_all || '')
    });
    console.log('Keybinds set:', get(keybinds));
}

// Check if a KeyboardEvent matches a keybind (supports Shift, Ctrl, Alt)
export function matchKeybind(event: KeyboardEvent, keybind: string) {
    // Empty keybind never matches
    if (!keybind || keybind.trim() === '') {
        return false;
    }

    const parts = keybind.split('+').map(p => p.trim());
    const baseKey = parts.pop()!.trim().toLowerCase();
    
    const shiftRequired = parts.some(p => p.toLowerCase() === 'shift');
    const ctrlRequired = parts.some(p => p.toLowerCase() === 'ctrl');
    const altRequired = parts.some(p => p.toLowerCase() === 'alt');

    // Normalize event.key for comparison
    let eventKey = event.key;

    // Handle special keys
    if (/^Arrow(Up|Down|Left|Right)$/i.test(eventKey)) {
        eventKey = eventKey; // Keep as-is
    } else if (eventKey === 'Enter') {
        eventKey = 'Enter';
    } else if (eventKey === 'Delete') {
        eventKey = 'Delete';
    } else {
        // For all other keys, lowercase for comparison
        eventKey = eventKey.toLowerCase();
    }

    // Compare keys
    const keysMatch = (baseKey === 'arrowup' || baseKey === 'arrowdown' || 
                       baseKey === 'arrowleft' || baseKey === 'arrowright')
        ? eventKey.toLowerCase() === baseKey
        : eventKey === baseKey;

    const modifiersMatch = (
        event.shiftKey === shiftRequired &&
        event.ctrlKey === ctrlRequired &&
        event.altKey === altRequired
    );

    const result = keysMatch && modifiersMatch;
    
    // Debug logging - remove this after testing
    if (eventKey === 'x') {
        console.log('X key debug:', {
            eventKey,
            baseKey,
            keybind,
            keysMatch,
            shiftRequired,
            'event.shiftKey': event.shiftKey,
            ctrlRequired,
            'event.ctrlKey': event.ctrlKey,
            altRequired,
            'event.altKey': event.altKey,
            modifiersMatch,
            result
        });
    }

    return result;
}

// Load keybinds from Rust backend
export async function loadKeybindsFromBackend() {
    try {
        const cfg = await invoke('get_claw_config');
        console.log('Config from backend:', cfg);
        setKeybindsFromConfig(cfg);
        console.log('Loaded keybinds from backend:', get(keybinds));
    } catch (err) {
        console.error('Failed to load keybinds from backend:', err);
    }
}
