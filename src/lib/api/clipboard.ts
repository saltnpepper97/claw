import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { currentClipboard, history, message, newText } from '$lib/stores/historyStore';

export interface ClipboardEntry {
    id: string;
    content?: number[];  // Changed from string to byte array
    timestamp: string;
    content_type: string;
}

export interface ClipboardData {
    content: number[];
    content_type: string;
}

export class ClipboardService {
    static async setClipboard(text: string): Promise<void> {
        return await invoke('set_system_clipboard', { text });
    }

    static async getClipboard(): Promise<ClipboardData> {
        return await invoke('get_system_clipboard');
    }

    static async getClipboardEntryContent(entryId: string): Promise<number[]> {
        return await invoke('get_clipboard_entry_content', { entryId });
    }

    static async getHistory(limit?: number): Promise<ClipboardEntry[]> {
        return await invoke('get_clipboard_history', { limit });
    }

    static async clearHistory(): Promise<void> {
        return await invoke('clear_clipboard_history');
    }

    static async removeEntry(entryId: string): Promise<boolean> {
        return await invoke('remove_clipboard_entry', { entryId });
    }

    static async setFromHistory(entryId: string): Promise<void> {
        return await invoke('set_clipboard_from_history', { entryId });
    }
}

// Helper to decode bytes to text
function bytesToText(bytes: number[]): string {
    const uint8Array = new Uint8Array(bytes);
    const decoder = new TextDecoder('utf-8');
    return decoder.decode(uint8Array);
}

export async function getCurrentClipboard() {
    try {
        const data = await ClipboardService.getClipboard();
        if (data.content_type === 'text') {
            currentClipboard.set(bytesToText(data.content));
        } else {
            currentClipboard.set(`[${data.content_type}]`);
        }
        message.set('Got current clipboard content');
    } catch (error) {
        message.set(`Failed to get clipboard: ${error}`);
    }
}

export async function loadHistory() {
    try {
        const entries = await ClipboardService.getHistory();

        history.set(entries);
    } catch (error) {
        message.set(`Failed to load history: ${error}`);
    }
}

export async function setClipboard() {
    if (!get(newText).trim()) return;
    
    try {
        await ClipboardService.setClipboard(get(newText));
        message.set('Clipboard set successfully!');
        newText.set('');
        await loadHistory();
    } catch (error) {
        message.set(`Failed to set clipboard: ${error}`);
    }
}

export async function useFromHistory(entry: ClipboardEntry) {
    try {
        // Load content from backend if not already loaded
        if (!entry.content || entry.content.length === 0) {
            entry.content = await ClipboardService.getClipboardEntryContent(entry.id);
        }

        await ClipboardService.setFromHistory(entry.id);

        let preview: string;
        if (entry.content_type.startsWith('image/')) {
            preview = `Image (${entry.content_type})`;
        } else {
            const text = bytesToText(entry.content);
            preview = truncate(text, 30);
        }

        message.set(`Set clipboard to: ${preview}`);
        await loadHistory();
    } catch (error) {
        message.set(`Failed to use from history: ${error}`);
    }
}


export async function removeFromHistory(entry: ClipboardEntry) {
    try {
        await ClipboardService.removeEntry(entry.id);
        message.set('Entry removed from history');
        await loadHistory();
    } catch (error) {
        message.set(`Failed to remove entry: ${error}`);
    }
}

export async function clearAllHistory() {
    try {
        await ClipboardService.clearHistory();
        message.set('History cleared');
        history.set([]);
    } catch (error) {
        message.set(`Failed to clear history: ${error}`);
    }
}

export function formatDate(timestamp: string) {
    return new Date(timestamp).toLocaleString();
}

export function truncate(text: string, length: number = 50) {
    return text.length > length ? text.substring(0, length) + '...' : text;
}
