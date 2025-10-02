<script lang="ts">
  import { ArrowCounterClockwise } from "phosphor-svelte";
  import { loadHistory } from "$lib/api/clipboard";
  import { history } from "$lib/stores/historyStore";
</script>

<div class="layout">
  <div class="left">
    <span class="hist-length">{$history.length} items</span>
  </div>
  <div class="right">
    <button class="refresh" onclick={loadHistory}>
      <ArrowCounterClockwise size={22} />
    </button>
  </div>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    height: 35px;
    background-color: var(--titlebar-background);
    user-select: none;
    -webkit-user-select: none;
  }

  .hist-length{
    color: var(--highlight);
    margin-left: 8px;
  }

  .right { 
    display: flex;
    gap: 10px;
  }

  span {
    flex-shrink: 0;
    border-radius: 4px;
    padding: 0 5px;
    background-color: var(--background-alt);
  }

  button {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    border-radius: 4px;
    margin-right: 5px;
    background-color: transparent;
    transition: background-color 130ms ease;
    color: var(--text-primary);
  }
  button:hover {
    background-color: var(--hover-titlebar);
  } 
</style>
