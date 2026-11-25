<script lang="ts">
  import PluginNeed from '../../../platform/PluginNeed.svelte'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { pluginEnabled } from '../../../platform/enabled.svelte'
  import MixReport from './MixReport.svelte'
</script>

{#if !pluginEnabled('pos')}
  <PluginNeed
    plugin="Point of Sale"
    purpose="Enable Point of Sale in Settings → Plugins so the product mix report can load."
  />
{:else if !pluginEnabled('inventory')}
  <PluginNeed
    plugin="Inventory"
    purpose="Enable Inventory in Settings → Plugins to open the product mix report."
  />
{:else}
  <ShopLocaleGate>
    {#snippet children(shop)}
      <MixReport currency={shop.currency} locale={shop.locale} />
    {/snippet}
  </ShopLocaleGate>
{/if}
