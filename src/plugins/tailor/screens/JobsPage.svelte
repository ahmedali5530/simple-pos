<script lang="ts">
  import ShopLocaleGate from '../../../platform/ShopLocaleGate.svelte'
  import JobsScreen from './JobsScreen.svelte'
</script>

<main class="stage">
  <ShopLocaleGate>
    {#snippet children(shop)}
      <JobsScreen currency={shop.currency} locale={shop.locale} />
    {/snippet}
  </ShopLocaleGate>
</main>
