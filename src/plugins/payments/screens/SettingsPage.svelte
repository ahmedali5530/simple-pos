<script lang="ts">
  import { onMount } from 'svelte'
  import { Button } from '../../../lib/ui'
  import {
    loadPaymentsSettings,
    savePaymentsSettings,
    type PaymentType,
    type PaymentsSettings,
  } from '../api'

  let settings = $state<PaymentsSettings>({ types: [] })
  let error = $state('')
  let saved = $state('')
  let busy = $state(false)

  async function refresh() {
    error = ''
    try {
      settings = await loadPaymentsSettings()
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load payment types.'
    }
  }

  function addType() {
    settings = {
      types: [
        ...settings.types,
        { id: '', label: '', enabled: true, requires_reference: false },
      ],
    }
  }

  function removeType(index: number) {
    settings = { types: settings.types.filter((_, i) => i !== index) }
  }

  function update(index: number, patch: Partial<PaymentType>) {
    settings = {
      types: settings.types.map((entry, i) => (i === index ? { ...entry, ...patch } : entry)),
    }
  }

  async function save() {
    busy = true
    error = ''
    saved = ''
    try {
      const cleaned: PaymentsSettings = {
        types: settings.types.map((entry) => ({
          ...entry,
          id: entry.id.trim().toLowerCase(),
          label: entry.label.trim(),
        })),
      }
      settings = await savePaymentsSettings(cleaned)
      saved = 'Saved.'
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not save payment types.'
    } finally {
      busy = false
    }
  }

  onMount(() => {
    void refresh()
  })
</script>

<main class="stage">
  <header>
    <h1>Payment types</h1>
    <p>Configure cash alternatives for the till: card, bank, wallets, or any third party.</p>
  </header>

  {#if error}<p class="error">{error}</p>{/if}
  {#if saved}<p class="ok">{saved}</p>{/if}

  <div class="list">
    {#each settings.types as entry, index (index)}
      <div class="row">
        <label>
          <span>Id</span>
          <input
            class="ui-input"
            placeholder="card"
            value={entry.id}
            oninput={(event) => update(index, { id: event.currentTarget.value })}
          />
        </label>
        <label>
          <span>Label</span>
          <input
            class="ui-input"
            placeholder="Card"
            value={entry.label}
            oninput={(event) => update(index, { label: event.currentTarget.value })}
          />
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={entry.enabled}
            onchange={(event) => update(index, { enabled: event.currentTarget.checked })}
          />
          Enabled
        </label>
        <label class="check">
          <input
            type="checkbox"
            checked={entry.requires_reference}
            onchange={(event) =>
              update(index, { requires_reference: event.currentTarget.checked })}
          />
          Needs reference
        </label>
        <Button variant="ghost" onclick={() => removeType(index)}>Remove</Button>
      </div>
    {:else}
      <p class="empty">No payment types yet.</p>
    {/each}
  </div>

  <div class="actions">
    <Button variant="secondary" onclick={addType}>Add type</Button>
    <Button disabled={busy} onclick={save}>{busy ? 'Saving…' : 'Save'}</Button>
  </div>
</main>

<style>
  .stage {
    padding: 1rem 1.25rem;
    display: grid;
    gap: 1rem;
  }
  header h1 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 600;
  }
  header p {
    margin: 0.35rem 0 0;
    color: var(--ui-text-muted);
  }
  .list {
    display: grid;
    gap: 0.75rem;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    align-items: end;
    padding: 0.75rem;
    border: 1px solid var(--ui-border);
    border-radius: var(--ui-radius);
    background: var(--ui-surface);
  }
  label {
    display: grid;
    gap: 0.25rem;
    font-size: 0.75rem;
    color: var(--ui-text-muted);
  }
  label .ui-input {
    min-width: 9rem;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding-bottom: 0.45rem;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .error {
    color: var(--ui-danger);
    margin: 0;
  }
  .ok {
    color: var(--ui-success);
    margin: 0;
  }
  .empty {
    color: var(--ui-text-muted);
  }
</style>
