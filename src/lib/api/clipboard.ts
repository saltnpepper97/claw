import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { currentClipboard, history, message, newText } from '$lib/stores/historyStore';

export interface ClipboardEntry {
    id: string;
    content: string;
    timestamp: string;
    content_type: string;
}

export class ClipboardService {
    static async setClipboard(text: string): Promise<void> {
        return await invoke('set_system_clipboard', { text });
    }

    static async getClipboard(): Promise<string> {
        return await invoke('get_system_clipboard');
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

export async function getCurrentClipboard() {
    try {
        currentClipboard.set(await ClipboardService.getClipboard());
        message.set('Got current clipboard content');
    } catch (error) {
        message.set(`Failed to get clipboard: ${error}`);
    }
}

export async function loadHistory() {
    try {
        history.set(await ClipboardService.getHistory(20));
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
        await loadHistory(); // Refresh history
    } catch (error) {
        message.set(`Failed to set clipboard: ${error}`);
    }
}


export async function useFromHistory(entry: ClipboardEntry) {
    try {
        await ClipboardService.setFromHistory(entry.id);
        message.set(`Set clipboard to: ${entry.content.substring(0, 30)}...`);
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

