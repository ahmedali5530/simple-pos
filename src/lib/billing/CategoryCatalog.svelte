<script lang="ts">
  import { orderCategories, removeCategory, saveCategory } from '../api'
  import type { Category, PosSnapshot } from '../types'

  interface Props {
    categories: Category[]
    counts: Record<number, number>
    onChange: (pos: PosSnapshot) => void
  }

  let { categories, counts, onChange }: Props = $props()

  let editing = $state<number | 'new' | null>(null)
  let loaded = $state<number | 'new' | null>(null)
  let name = $state('')
  let formError = $state('')
  let confirmRemove = $state(false)
  let saving = $state(false)

  $effect(() => {
    if (editing === loaded) return
    loaded = editing
    formError = ''
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
        formError = error instanceof Error ? error.message : 'Could not reorder the categories.'
      })
      .finally(() => {
        saving = false
      })
  }

  async function save() {
    if (!name.trim()) {
      formError = 'Enter a category name.'
      return
    }
    formError = ''
    saving = true
    try {
      const saved = await saveCategory(editing === 'new' ? null : editing, name.trim())
      onChange(saved.pos)
      loaded = saved.id
      editing = saved.id
    } catch (error) {
      formError = error instanceof Error ? error.message : 'Could not save the category.'
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
    } catch (error) {
      formError = error instanceof Error ? error.message : 'Could not remove the category.'
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
      <button type="button" class="save" onclick={() => (editing = 'new')}>New category</button>
    </div>
    <p class="hint">Categories filter the catalog inside a section. An item belongs to one category.</p>
    <div class="setup-list">
      {#each categories as category, index (category.id)}
        <div class="order-row">
          <button type="button" class:selected={category.id === editing} onclick={() => (editing = category.id)}>
            <strong>{category.name}</strong>
            <span>{counts[category.id] === 1 ? '1 item' : `${counts[category.id] ?? 0} items`}</span>
          </button>
          <button type="button" class="icon-btn" aria-label="Move {category.name} up" disabled={index === 0 || saving} onclick={() => move(index, -1)}>↑</button>
          <button type="button" class="icon-btn" aria-label="Move {category.name} down" disabled={index === categories.length - 1 || saving} onclick={() => move(index, 1)}>↓</button>
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
        <input bind:value={name} />
      </label>
      {#if formError}<p class="field-error">{formError}</p>{/if}
      <div class="sheet-actions">
        {#if editing !== 'new'}
          {#if confirmRemove}
            <button type="button" class="link" disabled={saving} onclick={remove}>Remove category</button>
            <button type="button" class="ghost light" disabled={saving} onclick={() => (confirmRemove = false)}>Cancel</button>
          {:else}
            <button type="button" class="link" disabled={saving} onclick={() => (confirmRemove = true)}>Remove</button>
          {/if}
        {/if}
        <button type="button" class="save" disabled={saving} onclick={save}>{saving ? 'Saving…' : 'Save'}</button>
      </div>
    {/if}
  </div>
</div>
