<script lang="ts">
    /* Libraries */
    import { getCurrentWindow } from '@tauri-apps/api/window';
    import { Minus, Square, X } from 'phosphor-svelte';


    let appWindow = $state(getCurrentWindow());

    function minimizeWindow() {
      appWindow.minimize();
    }

    function maximizeWindow() {
      appWindow.toggleMaximize();
    }

    function closeWindow() {
      appWindow.close();
    }
</script>

<div class="titlebar">
    <div data-tauri-drag-region></div>
    <div class="controls">
      <button id="titlebar-minimize" title="minimize" onclick={minimizeWindow} aria-label="minimize" tabindex="-1">
      <Minus weight="regular" size={20} style="margin-top: 10px;" />

      </button>
      <button id="titlebar-maximize" title="maximize" onclick={maximizeWindow} aria-label="maximize" tabindex="-1"> 
          <Square weight="regular" size={20} />
      </button>
      <button id="titlebar-close" title="close" onclick={closeWindow} aria-label="close" tabindex="-1">
          <X weight="regular" size={20} />
      </button>
    </div>
</div>



<style>
    .titlebar {
      background-color: var(--titlebar-background);
      display: grid;
      grid-template-columns: auto max-content;
      padding: 4px;
      user-select: none;
      -webkit-user-select: none;
    }
    .titlebar > .controls {
      display: flex;
    }
    .titlebar button {
      appearance: none;
      padding: 0;
      margin: 0;
      border: none;
      border-radius: 6px;
      display: inline-flex;
      justify-content: center;
      align-items: center;
      width: 30px;
      background-color: transparent;
      transition: background-color 130ms ease;
      color: var(--text-primary);
    }
    .titlebar button:hover {
      background-color: var(--hover-titlebar);
    }
</style>
