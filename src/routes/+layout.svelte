<script lang="ts">
  /* Root Styles */
  import '../app.css';
  /* API */
  import { setKeybindsFromConfig } from '$lib/api/keybinds';
  /* Libraries */
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  /* Assets */
  import favicon from '$lib/assets/favicon.svg';
  /* Components */
  import Titlebar from '$lib/components/Titlebar.svelte';
  import Statusbar from '$lib/components/Statusbar.svelte';
  /* Stores */
  import { showTitlebar, isDarkMode } from '$lib/stores/uiStore';
  /* Props */
  let { children } = $props();
 
  interface ThemeColors {
    background: string;
    'background-alt': string;
    'titlebar-background': string;
    'text-primary': string;
    'text-secondary': string;
    hover: string;
    'hover-titlebar': string;
    selected: string;
    'selected-foreground': string;
    highlight: string;
    outline: string;
  }
  
  interface Theme {
    light: ThemeColors;
    dark: ThemeColors;
  }
  
  interface ClawConfig {
    enable_titlebar: boolean;
    force_dark_mode: boolean;
    // Add other config properties as needed
  }
  
  let theme = $state<Theme | null>(null);
  
  function applyTheme(themeData: Theme, dark: boolean) {
    const targetTheme = dark ? themeData.dark : themeData.light;
    Object.entries(targetTheme).forEach(([key, value]) => {
      const colorValue = value.startsWith('#') ? value : `#${value}`;
      document.documentElement.style.setProperty(`--${key}`, colorValue);
    });
  }
  
  // Reactively apply theme when isDarkMode or theme changes
  $effect(() => {
    if (theme) {
      applyTheme(theme, $isDarkMode);
    }
  });
  
 
  onMount(() => {
    (async () => {
      // --- Fetch initial config from Rust ---
      let initialConfig: ClawConfig;
      try {
        initialConfig = await invoke<ClawConfig>('get_claw_config');
        showTitlebar.set(initialConfig.enable_titlebar);
        isDarkMode.set(initialConfig.force_dark_mode
          ? true
          : window.matchMedia('(prefers-color-scheme: dark)').matches);
        // Apply keybinds immediately
        setKeybindsFromConfig(initialConfig);
        // Fetch the theme from Rust
        theme = await invoke<Theme>('get_theme');
      } catch (err) {
        console.error('Failed to fetch initial config or theme:', err);
      }
      // Listen for dark mode changes
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleChange = (e: MediaQueryListEvent) => {
        if (initialConfig && !initialConfig.force_dark_mode) {
          isDarkMode.set(e.matches);
        }
      };
      mediaQuery.addEventListener('change', handleChange);
      // Listen for config reloads
      const unlisten = await listen('config-reloaded', async () => {
        try {
          const updatedConfig = await invoke<ClawConfig>('get_claw_config');
          // Update keybinds
          setKeybindsFromConfig(updatedConfig);
          // Update titlebar and dark mode
          showTitlebar.set(updatedConfig.enable_titlebar);
          isDarkMode.set(updatedConfig.force_dark_mode
            ? true
            : window.matchMedia('(prefers-color-scheme: dark)').matches);
          // Fetch the theme again
          theme = await invoke<Theme>('get_theme');
        } catch (err) {
          console.error('Failed to reload config or theme:', err);
        }
      });
      // Cleanup
      return () => {
        mediaQuery.removeEventListener('change', handleChange);
        unlisten();
      };
    })();
  });
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
</svelte:head>

<div class="layout" class:no-titlebar={!$showTitlebar}>
  <div class="titlebar-wrapper">
    {#if $showTitlebar}
      <div 
        class="titlebar-content"
        transition:slide={{ duration: 250, easing: cubicOut, axis: 'y' }}
      >
        <Titlebar />
      </div>
    {/if}
  </div>
  <Statusbar />
  {@render children?.()}
</div>

<style>
  .layout {
    display: grid;
    grid-template-rows: auto auto 1fr;
    height: 100%;
  }

  .titlebar-wrapper {
    overflow: hidden;
    min-height: 0;
  }

  .titlebar-content {
    will-change: transform, opacity;
  }
</style>
