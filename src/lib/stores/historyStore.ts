import type { ClipboardEntry } from "$lib/api/clipboard";
import { writable } from "svelte/store";

export let history = writable<ClipboardEntry[]>([]);
export let currentClipboard = writable('');
export let message = writable<string>('');
export let newText = writable<string>('');
