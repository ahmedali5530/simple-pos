<script lang="ts">
  import PluginNeed from '../../../platform/PluginNeed.svelte'
  import { pluginEnabled } from '../../../platform/enabled.svelte'
  import DevicesScreen from './DevicesScreen.svelte'
</script>

{#if !pluginEnabled('catalog')}
  <PluginNeed
    plugin="Catalog"
    purpose="Enable Catalog in Settings → Plugins so phone models can be linked to items."
  />
{:else}
  <main class="stage">
    <DevicesScreen />
  </main>
{/if}
