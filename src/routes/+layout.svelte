<script lang="ts">
  /* Root Styles */
  import '../app.css';

  /* API */
  import { setKeybindsFromConfig } from '$lib/api/keybinds';

  /* Libraries */
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  /* Assets */
  import favicon from '$lib/assets/favicon.svg';

  /* Components */
  import Titlebar from '$lib/components/Titlebar.svelte';
  import Statusbar from '$lib/components/Statusbar.svelte';

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
    highlight: string;
    outline: string;
  }
  
  interface Theme {
    light: ThemeColors;
    dark: ThemeColors;
  }
  
  let theme = $state<Theme | null>(null);
  let isDark = $state(false);
  let showTitlebar = $state(false);
  
  function applyTheme(themeData: Theme, dark: boolean) {
    const targetTheme = dark ? themeData.dark : themeData.light;
    Object.entries(targetTheme).forEach(([key, value]) => {
      const colorValue = value.startsWith('#') ? value : `#${value}`;
      document.documentElement.style.setProperty(`--${key}`, colorValue);
    });
    console.log('Applied theme:', dark ? 'dark' : 'light', targetTheme);
  }
  
  // Reactively apply theme when isDark or theme changes
  $effect(() => {
    if (theme) {
      applyTheme(theme, isDark);
    }
  });
  
 


onMount(async () => {
  console.log('Layout mounted, setting up theme...');

  // --- Fetch initial config from Rust ---
  let initialConfig;
  try {
    initialConfig = await invoke('get_claw_config');
    console.log('Initial config fetched:', initialConfig);

    showTitlebar = initialConfig.enable_titlebar;

    isDark = initialConfig.force_dark_mode
      ? true
      : window.matchMedia('(prefers-color-scheme: dark)').matches;

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
    if (!theme?.force_dark_mode) {
      isDark = e.matches;
      console.log('Dark mode changed:', isDark);
    }
  };
  mediaQuery.addEventListener('change', handleChange);

  // Listen for config reloads
  const unlisten = listen('config-reloaded', async () => {
    console.log('Config reloaded event received!');

    try {
      const updatedConfig = await invoke('get_claw_config');

      // Update keybinds
      setKeybindsFromConfig(updatedConfig);

      // Update titlebar and dark mode
      showTitlebar = updatedConfig.enable_titlebar;
      isDark = updatedConfig.force_dark_mode
        ? true
        : window.matchMedia('(prefers-color-scheme: dark)').matches;

      // Fetch the theme again
      theme = await invoke<Theme>('get_theme');
    } catch (err) {
      console.error('Failed to reload config or theme:', err);
    }
  });

  // Cleanup
  return () => {
    mediaQuery.removeEventListener('change', handleChange);
    unlisten.then(f => f());
  };
});


</script>

<svelte:head>
  <link rel="icon" href={favicon} />
</svelte:head>

<div class="layout">
  {#if showTitlebar}
    <Titlebar />
  {/if}
  <Statusbar />
  {@render children?.()}
</div>

<style>
  .layout {
    display: grid;
    grid-template-rows: auto auto 1fr;
    height: 100%;
  }
</style>
