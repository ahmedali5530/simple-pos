<script lang="ts">
  import PluginNeed from '../../../platform/PluginNeed.svelte'
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import { pluginEnabled } from '../../../platform/enabled.svelte'
  import WarrantyScreen from './WarrantyScreen.svelte'
</script>

{#if !pluginEnabled('pos')}
  <PluginNeed
    plugin="Point of Sale"
    purpose="Enable Point of Sale in Settings → Plugins so warranties can follow sales."
  />
{:else}
  <main class="stage">
    <ShopLocaleGate>
      {#snippet children(shop)}
        <WarrantyScreen currency={shop.currency} locale={shop.locale} />
      {/snippet}
    </ShopLocaleGate>
  </main>
{/if}
