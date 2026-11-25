<script lang="ts">
  import type { CatalogItem } from '../api'
  import { followLink, type SettingsPage } from '../route.svelte'
  import type { PosItem, PosSnapshot } from '../types'
  import CategoryCatalog from './CategoryCatalog.svelte'
  import ItemCatalog from './ItemCatalog.svelte'
  import SectionCatalog from './SectionCatalog.svelte'
  import SystemSettings from './SystemSettings.svelte'
  import UserCatalog from './UserCatalog.svelte'

  interface Props {
    snapshot: PosSnapshot
    page: SettingsPage
    saving: boolean
    onSave: (
      itemId: number | null,
      body: CatalogItem,
      picture: { file: File | null; remove: boolean },
    ) => Promise<{ item: PosItem; pictureError: string | null }>
    onChange: (pos: PosSnapshot) => void
  }

  let { snapshot, page, saving, onSave, onChange }: Props = $props()

  const pages: { id: SettingsPage; label: string }[] = [
    { id: 'items', label: 'Items' },
    { id: 'sections', label: 'Sections' },
    { id: 'categories', label: 'Categories' },
    { id: 'users', label: 'Users' },
    { id: 'system', label: 'System' },
  ]

  const sectionCounts = $derived(
    Object.fromEntries(
      snapshot.all_sections.map((section) => [
        section.id,
        snapshot.sections.find((entry) => entry.id === section.id)?.items.length ?? 0,
      ]),
    ),
  )
  const categoryCounts = $derived(
    Object.fromEntries(
      snapshot.categories.map((category) => [
        category.id,
        snapshot.sections.reduce(
          (sum, section) => sum + section.items.filter((item) => item.category_id === category.id).length,
          0,
        ),
      ]),
    ),
  )
</script>

<div class="settings">
  <nav class="settings-nav" aria-label="Settings">
    {#each pages as entry (entry.id)}
      <a
        href="/settings/{entry.id}"
        class:active={page === entry.id}
        onclick={(event) => followLink(event, `/settings/${entry.id}`)}
      >
        {entry.label}
      </a>
    {/each}
  </nav>
  <div class="settings-body">
    {#if page === 'items'}
      <ItemCatalog
        sections={snapshot.sections}
        categories={snapshot.categories}
        currency={snapshot.currency}
        locale={snapshot.locale}
        {saving}
        {onSave}
        onImported={onChange}
        onRemoved={onChange}
      />
    {:else if page === 'sections'}
      <SectionCatalog sections={snapshot.all_sections} counts={sectionCounts} {onChange} />
    {:else if page === 'categories'}
      <CategoryCatalog categories={snapshot.categories} counts={categoryCounts} {onChange} />
    {:else if page === 'users'}
      <UserCatalog users={snapshot.users ?? []} {onChange} />
    {:else}
      <SystemSettings {snapshot} {onChange} />
    {/if}
  </div>
</div>
