<script lang="ts">
  import { orderSections, removeSection, saveSection } from '../api'
  import type { PosSnapshot, SectionChoice, SectionKind } from '../types'
  import { toast, toastError, toastSaved } from '../toast'

  interface Props {
    sections: SectionChoice[]
    counts: Record<number, number>
    onChange: (pos: PosSnapshot) => void
  }

  let { sections, counts, onChange }: Props = $props()

  let editing = $state<number | 'new' | null>(null)
  let loaded = $state<number | 'new' | null>(null)
  let name = $state('')
  let shortName = $state('')
  let kind = $state<SectionKind>('retail')
  let accent = $state('#0f766e')
  let confirmRemove = $state(false)
  let saving = $state(false)

  const selected = $derived(sections.find((section) => section.id === editing) ?? null)

  $effect(() => {
    if (editing === loaded) return
    loaded = editing
    confirmRemove = false
    const section = sections.find((entry) => entry.id === editing) ?? null
    name = section?.name ?? ''
    shortName = section?.short_name ?? ''
    kind = section?.kind ?? 'retail'
    accent = section?.accent ?? '#0f766e'
  })

  function move(index: number, direction: -1 | 1) {
    const next = index + direction
    if (next < 0 || next >= sections.length || saving) return
    const ids = sections.map((section) => section.id)
    const [id] = ids.splice(index, 1)
    ids.splice(next, 0, id)
    saving = true
    orderSections(ids)
      .then(onChange)
      .catch((error) => {
        toastError(error, 'Could not reorder the sections.')
      })
      .finally(() => {
        saving = false
      })
  }

  async function save() {
    if (!name.trim()) {
      toast.error('Enter a section name.')
      return
    }
    if (!shortName.trim()) {
      toast.error('Enter the short name shown on the tab.')
      return
    }
    saving = true
    try {
      const saved = await saveSection(editing === 'new' ? null : editing, {
        name: name.trim(),
        short_name: shortName.trim(),
        kind,
        accent,
      })
      onChange(saved.pos)
      loaded = saved.id
      editing = saved.id
      toastSaved('Section saved.')
    } catch (error) {
      toastError(error, 'Could not save the section.')
    } finally {
      saving = false
    }
  }

  async function remove() {
    if (editing === 'new' || editing == null) return
    saving = true
    try {
      onChange(await removeSection(editing))
      loaded = null
      editing = null
      toastSaved('Section removed.')
    } catch (error) {
      toastError(error, 'Could not remove the section.')
      confirmRemove = false
    } finally {
      saving = false
    }
  }
</script>

<div class="item-setup">
  <div class="setup-list-pane">
    <div class="catalog-head">
      <h2>Sections</h2>
      <button type="button" class="save" onclick={() => (editing = 'new')}>New section</button>
    </div>
    <p class="hint">Each section is its own bill. Tabs follow this order, and F1–F8 follow the first eight.</p>
    <div class="setup-list">
      {#each sections as section, index (section.id)}
        <div class="order-row">
          <button type="button" class:selected={section.id === editing} onclick={() => (editing = section.id)}>
            <span class="swatch" style:background={section.accent}></span>
            <strong>{section.short_name}</strong>
            <span>{counts[section.id] === 1 ? '1 item' : `${counts[section.id] ?? 0} items`}</span>
          </button>
          <button type="button" class="icon-btn" aria-label="Move {section.short_name} up" disabled={index === 0 || saving} onclick={() => move(index, -1)}>↑</button>
          <button type="button" class="icon-btn" aria-label="Move {section.short_name} down" disabled={index === sections.length - 1 || saving} onclick={() => move(index, 1)}>↓</button>
        </div>
      {/each}
    </div>
  </div>
  <div class="setup-detail">
    {#if editing == null}
      <h2>Section</h2>
      <p class="hint">Select a section, or add one. The short name is the tab on the till.</p>
    {:else}
      <h2>{editing === 'new' ? 'New section' : shortName || 'Section'}</h2>
      {#if selected}
        <p class="hint">{selected.name}</p>
      {/if}
      <label>
        Name
        <input bind:value={name} />
      </label>
      <label>
        Short name
        <input bind:value={shortName} />
      </label>
      <div class="prompt-choices" role="radiogroup" aria-label="Section kind">
        <button type="button" role="radio" class:active={kind === 'retail'} aria-checked={kind === 'retail'} onclick={() => (kind = 'retail')}>Retail</button>
        <button type="button" role="radio" class:active={kind === 'food'} aria-checked={kind === 'food'} onclick={() => (kind = 'food')}>Food</button>
      </div>
      <label class="color-field">
        Color
        <input type="color" bind:value={accent} />
      </label>
      <div class="sheet-actions">
        {#if editing !== 'new'}
          {#if confirmRemove}
            <button type="button" class="link" disabled={saving} onclick={remove}>Remove section</button>
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
