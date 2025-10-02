<script lang="ts">
    import { onMount } from 'svelte';
    import { history } from '$lib/stores/historyStore';
    import { loadHistory, useFromHistory, removeFromHistory, truncate, formatDate, clearAllHistory } from '$lib/api/clipboard';
    import { keybinds, loadKeybindsFromBackend, matchKeybind } from '$lib/api/keybinds';
    import { get } from 'svelte/store';
    import { listen } from '@tauri-apps/api/event';

    let selectedIndex = $state(-1);
    let historyContainer: HTMLElement | null = $state(null);
    let unlisten = $state();

    // Function to scroll the selected item into view
    function scrollToSelected() {
        if (historyContainer && selectedIndex >= 0) {
            const selectedItem = historyContainer.querySelector(`[data-index="${selectedIndex}"]`) as HTMLElement;
            if (selectedItem) {
                selectedItem.scrollIntoView({
                    behavior: 'smooth',
                    block: 'nearest',
                    inline: 'nearest'
                });
            }
        }
    }

    function handleKeyDown(event: KeyboardEvent) {
        const kb = get(keybinds);
        const previousIndex = selectedIndex;

        if (matchKeybind(event, kb.historyUp)) {
            selectedIndex = Math.max(selectedIndex - 1, 0);
            event.preventDefault();
        } else if (matchKeybind(event, kb.historyDown)) {
            selectedIndex = Math.min(selectedIndex + 1, $history.length - 1);
            event.preventDefault();
        } else if (matchKeybind(event, kb.useEntry) && selectedIndex >= 0) {
            useFromHistory($history[selectedIndex]);
            event.preventDefault();
        } else if (matchKeybind(event, kb.removeEntry) && selectedIndex >= 0) {
            removeFromHistory($history[selectedIndex]);
            selectedIndex = Math.min(selectedIndex, $history.length - 2);
            event.preventDefault();
        } else if (matchKeybind(event, kb.deleteAll)) {
            clearAllHistory();
            selectedIndex = -1;
            event.preventDefault();
        }

        if (selectedIndex !== previousIndex) {
            setTimeout(scrollToSelected, 0);
        }
    }

    onMount(() => {
        loadKeybindsFromBackend();
        loadHistory();
        // Select first item in the list the list isn't empty
        if ($history.length > 0 && selectedIndex == -1) {
          selectedIndex = 0;
          setTimeout(scrollToSelected, 0);
        }
        
        unlisten = listen<string>('history-updated', () => {
            loadHistory();
        });
    });
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if $history.length === 0}
    <div class="empty">
      <p class="empty-txt">No clipboard history yet. Copy something to get started!</p>
    </div>
{:else}  
    <div class="history-list" bind:this={historyContainer}>
        {#each $history as entry, i (entry.id)}
            <div class="history-item {selectedIndex === i ? 'selected' : ''}" data-index={i} onclick={() => { selectedIndex = i; scrollToSelected(); }}>
                <div class="content">
                    <div class="text">{truncate(entry.content)}</div>
                    <div class="meta">
                        <span class="date">{formatDate(entry.timestamp)}</span>
                        <span class="type">{entry.content_type}</span>
                    </div>
                </div>
            </div>
        {/each}
    </div>
{/if}

<style>
    .history-item.selected {
        border-color: var(--outline);
        background-color: var(--selected);
    }
     
    .empty {
        display: flex;
        align-items: center;
        justify-content: center;
        height: 100%;
    }

    .empty-txt {
        text-align: center;
        color: var(--text-secondary);
        font-style: italic;
        padding: 20px;
    }

    .history-list {
        display: flex;
        flex-direction: column;
        overflow-y: auto;
        user-select: none;
        -webkit-user-select: none;
        scroll-behavior: smooth;
    }

    .history-item {
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
        background: var(--background);
        border-radius: 0px;
        padding: 14px 12px;
        box-shadow: 0 1px 3px rgba(0,0,0,0.08);
        border-style: solid;
        border-color: transparent;
        border-width: 1px 0 1px 0;
        transition: all 200ms cubic-bezier(0.4, 0, 0.2, 1);
        position: relative;
    }

    .history-item:hover {
        border-color: var(--outline);
        background-color: var(--hover);
    }

    .history-item:hover.selected {
        border-color: var(--outline);
        background-color: var(--selected);
    }

    .content {
        flex: 1;
        margin-right: 0px;
        transition: all ease 200ms;
    }

    .text {
        font-family: monospace;
        padding: 5px 8px;
        color: var(--text-primary);
        border-radius: 3px;
        margin-bottom: 8px;
        word-break: break-all;
    }

    .meta {
        display: flex;
        gap: 15px;
        font-size: 0.85em;
        color: var(--text-secondary);
    }

    .type {
        border-radius: 3px;
        height: 20px;
        padding: 0 5px;
        font-weight: 500;
        background-color: var(--background-alt);
        color: var(--highlight)
    }

    @media (max-width: 600px) {
        .history-item {
            flex-direction: column;
            align-items: stretch;
        }
        
        .content {
            margin-right: 0;
            margin-bottom: 10px;
        }
    }
</style>
