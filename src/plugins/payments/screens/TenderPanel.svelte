<script lang="ts">
  import { onMount } from 'svelte'
  import type { PosTenderContext } from '../../../platform/slots'
  import { loadPaymentsTypes, type PaymentType } from '../api'

  let { total, tenders, setTenders }: PosTenderContext = $props()

  let types = $state<PaymentType[]>([])
  let error = $state('')
  let selectedId = $state('cash')
  let reference = $state('')

  // Cash is always the built-in first button — skip a configured `cash` type.
  const enabled = $derived(
    types.filter((entry) => entry.enabled && entry.id.toLowerCase() !== 'cash'),
  )
  const selected = $derived(enabled.find((entry) => entry.id === selectedId) ?? null)
  const otherPaid = $derived(
    tenders
      .filter((tender) => !tender.method.startsWith('payments.') && tender.method !== 'cash')
      .reduce((sum, tender) => sum + tender.amount, 0),
  )
  const payable = $derived(Math.max(0, total - otherPaid))

  function methodFor(id: string) {
    return `payments.${id}`
  }

  function withoutPayments(list = tenders) {
    return list.filter(
      (tender) => !tender.method.startsWith('payments.') && tender.method !== 'cash',
    )
  }

  function applySelected(id: string, ref: string) {
    error = ''
    if (id === 'cash') {
      setTenders(withoutPayments())
      return
    }
    const entry = enabled.find((type) => type.id === id)
    if (!entry) return
    if (payable < 1) {
      error = 'Nothing left to pay with this method.'
      setTenders(withoutPayments())
      return
    }
    if (entry.requires_reference && !ref.trim()) {
      setTenders(withoutPayments())
      return
    }
    setTenders([
      ...withoutPayments(),
      {
        method: methodFor(entry.id),
        amount: payable,
        reference: ref.trim(),
        details: { kind: 'payments.apply', type_id: entry.id, label: entry.label },
      },
    ])
  }

  function select(id: string) {
    selectedId = id
    if (id === 'cash') {
      reference = ''
      applySelected('cash', '')
      return
    }
    const entry = enabled.find((type) => type.id === id)
    if (!entry?.requires_reference) reference = ''
    applySelected(id, reference)
  }

  function onReferenceInput(value: string) {
    reference = value
    if (selectedId === 'cash') return
    applySelected(selectedId, value)
  }

  // Keep the active payments tender amount in sync when the cart total changes.
  $effect(() => {
    if (selectedId === 'cash') return
    const entry = enabled.find((type) => type.id === selectedId)
    if (!entry) return
    if (entry.requires_reference && !reference.trim()) return
    const method = methodFor(selectedId)
    const current = tenders.find((tender) => tender.method === method)
    if (!current || current.amount === payable) return
    applySelected(selectedId, reference)
  })

  onMount(async () => {
    try {
      const loaded = await loadPaymentsTypes()
      types = loaded.types
      const current = tenders.find((tender) => tender.method.startsWith('payments.'))
      if (current) {
        selectedId = current.method.slice('payments.'.length)
        reference = current.reference ?? ''
      }
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load payment types.'
    }
  })
</script>

{#if enabled.length > 0}
  <div class="payments-tender">
    <div class="methods" role="group" aria-label="Payment method">
      <button
        type="button"
        class="method"
        class:active={selectedId === 'cash'}
        aria-pressed={selectedId === 'cash'}
        onclick={() => select('cash')}
      >
        Cash
      </button>
      {#each enabled as entry (entry.id)}
        <button
          type="button"
          class="method"
          class:active={selectedId === entry.id}
          aria-pressed={selectedId === entry.id}
          onclick={() => select(entry.id)}
        >
          {entry.label}
        </button>
      {/each}
    </div>

    {#if selected?.requires_reference}
      <input
        class="ui-input reference"
        placeholder="Reference / auth code"
        value={reference}
        oninput={(event) => onReferenceInput(event.currentTarget.value)}
      />
    {/if}

    {#if error}<p class="hint">{error}</p>{/if}
  </div>
{/if}

<style>
  .payments-tender {
    display: grid;
    gap: 0.4rem;
    margin: 0.35rem 0;
  }
  .methods {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }
  .method {
    appearance: none;
    border: 1px solid var(--ui-border);
    background: var(--ui-surface);
    color: var(--ui-text);
    border-radius: var(--ui-radius-sm, 0.4rem);
    min-height: var(--ui-h-md, 2.75rem);
    padding: 0.65rem 1.1rem;
    font: inherit;
    font-size: 1.05rem;
    font-weight: 500;
    cursor: pointer;
  }
  .method.active {
    border-color: transparent;
    background:
      linear-gradient(180deg, oklch(1 0 0 / 0.22), oklch(1 0 0 / 0) 55%),
      var(--accent, var(--ui-primary));
    color: white;
  }
  .reference {
    width: 100%;
    min-height: var(--ui-h-md, 2.75rem);
    font-size: 1.05rem;
  }
  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--ui-danger);
  }
</style>
