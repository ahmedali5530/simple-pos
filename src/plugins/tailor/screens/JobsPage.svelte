<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import JobsScreen from './JobsScreen.svelte'
</script>

<main class="stage">
  <ShopGate>
    {#snippet children(snapshot)}
      <JobsScreen currency={snapshot.currency} locale={snapshot.locale} />
    {/snippet}
  </ShopGate>
</main>
