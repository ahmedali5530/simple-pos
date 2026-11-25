<script lang="ts">
  import PluginNeed from '../../../platform/PluginNeed.svelte'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { pluginEnabled } from '../../../platform/enabled.svelte'
  import SalesReport from './SalesReport.svelte'
</script>

{#if !pluginEnabled('pos')}
  <PluginNeed
    plugin="Point of Sale"
    purpose="Enable Point of Sale in Settings → Plugins to open the sales report."
  />
{:else if !pluginEnabled('inventory')}
  <PluginNeed
    plugin="Inventory"
    purpose="Enable Inventory in Settings → Plugins so the sales report can include stock figures."
  />
{:else}
  <ShopLocaleGate>
    {#snippet children(shop)}
      <SalesReport currency={shop.currency} locale={shop.locale} />
    {/snippet}
  </ShopLocaleGate>
{/if}
