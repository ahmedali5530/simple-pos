<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import CategoryCatalog from './CategoryCatalog.svelte'
</script>

<ShopGate>
  {#snippet children(snapshot, setSnapshot)}
    <CategoryCatalog
      categories={snapshot.categories}
      counts={Object.fromEntries(
        snapshot.categories.map((category) => [
          category.id,
          snapshot.sections.reduce((sum, section) => sum + section.items.filter((item) => item.category_id === category.id).length, 0),
        ]),
      )}
      onChange={setSnapshot}
    />
  {/snippet}
</ShopGate>
