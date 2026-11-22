<script lang="ts">
  import type { SectionChoice } from '../types'
  import { Button, Switch } from '../ui'

  interface Props {
    sections: SectionChoice[]
    onClose: () => void
    onSave: (sections: SectionChoice[]) => void
    saving: boolean
  }

  let { sections, onClose, onSave, saving }: Props = $props()

  let rows = $state<SectionChoice[]>([])

  $effect(() => {
    rows = sections.map((section) => ({ ...section }))
  })

  const shown = $derived(rows.filter((section) => section.show_on_pos).length)

  function move(index: number, direction: -1 | 1) {
    const next = index + direction
    if (next < 0 || next >= rows.length) return
    const copy = [...rows]
    const [item] = copy.splice(index, 1)
    copy.splice(next, 0, item)
    rows = copy
  }
</script>

<div
  class="sheet-backdrop"
  role="presentation"
  onclick={onClose}
  onkeydown={(event) => {
    if (event.key === 'Escape') onClose()
  }}
>
  <div
    class="sheet"
    role="dialog"
    aria-labelledby="section-layout-title"
    tabindex="-1"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => event.stopPropagation()}
  >
    <h2 id="section-layout-title">Sections on this till</h2>
    <p class="hint">
      Turn on one, two, or as many sections as this screen should show. Each section keeps its own bill.
      Order here is the order across the till.
    </p>
    {#each rows as section, index (section.id)}
      <div class="layout-row">
        <Switch
          checked={section.show_on_pos}
          ariaLabel="Show {section.short_name} on the till"
          onchange={() => {
            rows[index].show_on_pos = !rows[index].show_on_pos
          }}
        />
        <div>
          <strong>{section.name}</strong>
          <div class="hint">{section.short_name}</div>
        </div>
        <div class="order">
          <Button variant="secondary" size="icon" aria-label="Move {section.short_name} up" onclick={() => move(index, -1)}>↑</Button>
          <Button variant="secondary" size="icon" aria-label="Move {section.short_name} down" onclick={() => move(index, 1)}>↓</Button>
        </div>
      </div>
    {/each}
    {#if shown === 0}
      <p class="field-error">Keep at least one section on the till.</p>
    {/if}
    <div class="sheet-actions">
      <Button variant="secondary" onclick={onClose}>Cancel</Button>
      <Button disabled={shown === 0 || saving} onclick={() => onSave(rows)}>
        {saving ? 'Saving…' : `Show ${shown} on the till`}
      </Button>
    </div>
  </div>
</div>
