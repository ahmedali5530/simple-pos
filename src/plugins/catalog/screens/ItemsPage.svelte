<script lang="ts">
  import ShopGate from '../../../platform/ShopGate.svelte'
  import ItemCatalog from './ItemCatalog.svelte'
  import { removeItemPicture, saveCatalogItem, saveItemPicture, type CatalogItem } from '../../../lib/api'
  import type { PosItem, PosSnapshot } from '../../../lib/types'

  let saving = $state(false)

  async function saveItem(
    snapshot: PosSnapshot,
    setSnapshot: (pos: PosSnapshot) => void,
    itemId: number | null,
    body: CatalogItem,
    picture: { file: File | null; remove: boolean },
  ) {
    saving = true
    try {
      const saved = await saveCatalogItem(itemId, body)
      let next = saved.pos
      let pictureError: string | null = null
      try {
        if (picture.remove && !picture.file) next = (await removeItemPicture(saved.item_id)).pos
        if (picture.file) next = (await saveItemPicture(saved.item_id, picture.file)).pos
      } catch (error) {
        pictureError = error instanceof Error ? error.message : 'Could not save the picture.'
      }
      setSnapshot(next)
      const item = next.sections.flatMap((section) => section.items).find((entry) => entry.id === saved.item_id)
      return { item: item as PosItem, pictureError }
    } finally {
      saving = false
    }
  }
</script>

<ShopGate>
  {#snippet children(snapshot, setSnapshot)}
    <ItemCatalog
      sections={snapshot.sections}
      categories={snapshot.categories}
      currency={snapshot.currency}
      locale={snapshot.locale}
      {saving}
      onSave={(itemId, body, picture) => saveItem(snapshot, setSnapshot, itemId, body, picture)}
      onImported={setSnapshot}
    />
  {/snippet}
</ShopGate>
