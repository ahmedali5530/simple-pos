<script lang="ts">
  import CatalogGate from '../../../platform/CatalogGate.svelte'
  import SectionCatalog from './SectionCatalog.svelte'
</script>

<CatalogGate>
  {#snippet children(snapshot, setSnapshot)}
    <SectionCatalog
      sections={snapshot.all_sections}
      counts={Object.fromEntries(snapshot.all_sections.map((section) => [section.id, snapshot.sections.find((entry) => entry.id === section.id)?.items.length ?? 0]))}
      onChange={setSnapshot}
    />
  {/snippet}
</CatalogGate>
