<script lang="ts">
  import { downloadWorkbook, importCategoriesWorkbook, loadCatalog, orderCategories, removeCategory, saveCategory } from '../../../lib/api'
  import type { Category, PosSnapshot } from '../../../lib/types'
  import { toast, toastError, toastSaved } from '../../../lib/toast'
  import { Button, Input } from '../../../lib/ui'

  interface Props {
    categories: Category[]
    counts: Record<number, number>
    onChange: (pos: PosSnapshot) => void
  }

  let { categories, counts, onChange }: Props = $props()

  let editing = $state<number | 'new' | null>(null)
  let loaded = $state<number | 'new' | null>(null)
  let name = $state('')
  let confirmRemove = $state(false)
  let saving = $state(false)
  let sheetBusy = $state(false)

  $effect(() => {
    if (editing === loaded) return
    loaded = editing
    confirmRemove = false
    name = categories.find((category) => category.id === editing)?.name ?? ''
  })

  function move(index: number, direction: -1 | 1) {
    const next = index + direction
    if (next < 0 || next >= categories.length || saving) return
    const ids = categories.map((category) => category.id)
    const [id] = ids.splice(index, 1)
    ids.splice(next, 0, id)
    saving = true
    orderCategories(ids)
      .then(onChange)
      .catch((error) => {
        toastError(error, 'Could not reorder the categories.')
      })
      .finally(() => {
        saving = false
      })
  }

  async function exportWorkbook() {
    sheetBusy = true
    try {
      await downloadWorkbook('/api/v1/catalog/categories/export', 'categories.xlsx')
      toastSaved('The category workbook downloaded.')
    } catch (error) {
      toastError(error, 'The workbook did not download.')
    } finally {
      sheetBusy = false
    }
  }

  async function importWorkbook(event: Event) {
    const input = event.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    sheetBusy = true
    try {
      const result = await importCategoriesWorkbook(file)
      const pos = await loadCatalog()
      onChange(pos)
      if (typeof editing === 'number' && !pos.categories.some((category) => category.id === editing)) {
        editing = null
      }
      loaded = null
      const parts = [
        result.created ? `${result.created} added` : '',
        result.updated ? `${result.updated} renamed` : '',
        result.removed ? `${result.removed} removed` : '',
      ].filter(Boolean)
      if (parts.length) toastSaved(`Saved the workbook: ${parts.join(', ')}.`)
      else if (result.reordered) toastSaved('Saved the category order.')
      else toastSaved('The workbook matched the catalog.')
    } catch (error) {
      toastError(error, 'The workbook was not saved.')
    } finally {
      sheetBusy = false
    }
  }

  async function save() {
    if (!name.trim()) {
      toast.error('Enter a category name.')
      return
    }
    saving = true
    try {
      const saved = await saveCategory(editing === 'new' ? null : editing, name.trim())
      onChange(saved.pos)
      loaded = saved.id
      editing = saved.id
      toastSaved('Category saved.')
    } catch (error) {
      toastError(error, 'Could not save the category.')
    } finally {
      saving = false
    }
  }

  async function remove() {
    if (editing === 'new' || editing == null) return
    saving = true
    try {
      onChange(await removeCategory(editing))
      loaded = null
      editing = null
      toastSaved('Category removed.')
    } catch (error) {
      toastError(error, 'Could not remove the category.')
      confirmRemove = false
    } finally {
      saving = false
    }
  }
</script>

<div class="item-setup">
  <div class="setup-list-pane">
    <div class="catalog-head">
      <h2>Categories</h2>
      <Button onclick={() => (editing = 'new')}>New category</Button>
    </div>
    <div class="mt-2.5 grid grid-cols-2 gap-2">
      <Button variant="secondary" class="w-full" disabled={sheetBusy || saving} onclick={exportWorkbook}>Export Excel</Button>
      <label class="btn btn-secondary w-full">
        Import Excel
        <input class="hidden" type="file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" disabled={sheetBusy || saving} onchange={importWorkbook} />
      </label>
    </div>
    <p class="hint">Categories filter the catalog inside a section. An item belongs to one category. Leave id blank to add one, and put yes in remove to delete one. Row order is the catalog order.</p>
    <div class="setup-list">
      {#each categories as category, index (category.id)}
        <div class="order-row">
          <button type="button" class:selected={category.id === editing} onclick={() => (editing = category.id)}>
            <strong>{category.name}</strong>
            <span>{counts[category.id] === 1 ? '1 item' : `${counts[category.id] ?? 0} items`}</span>
          </button>
          <Button variant="secondary" size="icon" aria-label="Move {category.name} up" disabled={index === 0 || saving} onclick={() => move(index, -1)}>↑</Button>
          <Button variant="secondary" size="icon" aria-label="Move {category.name} down" disabled={index === categories.length - 1 || saving} onclick={() => move(index, 1)}>↓</Button>
        </div>
      {/each}
    </div>
  </div>
  <div class="setup-detail">
    {#if editing == null}
      <h2>Category</h2>
      <p class="hint">Select a category, or add one.</p>
    {:else}
      <h2>{editing === 'new' ? 'New category' : name || 'Category'}</h2>
      <label>
        Name
        <Input bind:value={name} />
      </label>
      <div class="sheet-actions">
        {#if editing !== 'new'}
          {#if confirmRemove}
            <Button variant="link" class="text-danger" disabled={saving} onclick={remove}>Remove category</Button>
            <Button variant="secondary" disabled={saving} onclick={() => (confirmRemove = false)}>Cancel</Button>
          {:else}
            <Button variant="link" class="text-danger" disabled={saving} onclick={() => (confirmRemove = true)}>Remove</Button>
          {/if}
        {/if}
        <Button disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</Button>
      </div>
    {/if}
  </div>
</div>
