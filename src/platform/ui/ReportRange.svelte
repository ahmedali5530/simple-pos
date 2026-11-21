<script lang="ts">
  import { onMount } from 'svelte'
  import { reportSpan, type ReportPreset, type ReportSpan } from '../range'

  interface Props {
    label?: string
    onchange: (span: Extract<ReportSpan, { kind: 'ready' }>) => void
    oninvalid?: (message: string) => void
  }

  let { label = '', onchange, oninvalid }: Props = $props()

  let preset = $state<ReportPreset>('today')
  let customFrom = $state('')
  let customTo = $state('')

  const presets: { id: ReportPreset; label: string }[] = [
    { id: 'today', label: 'Today' },
    { id: 'yesterday', label: 'Yesterday' },
    { id: 'thisWeek', label: 'This week' },
    { id: 'lastWeek', label: 'Last week' },
    { id: 'thisMonth', label: 'This month' },
    { id: 'all', label: 'All' },
    { id: 'custom', label: 'Custom range' },
  ]

  function publish() {
    const span = reportSpan(preset, customFrom, customTo)
    if (span.kind === 'incomplete') return
    if (span.kind === 'invalid') {
      oninvalid?.('The end date is before the start date.')
      return
    }
    onchange(span)
  }

  function choose(next: ReportPreset) {
    preset = next
    if (next !== 'custom') publish()
  }

  onMount(publish)
</script>

<div class="flex flex-none flex-wrap items-center gap-2">
  {#each presets as choice (choice.id)}
    <button
      type="button"
      class="inline-flex h-10 items-center rounded-xl px-3 text-sm font-extrabold {preset === choice.id ? 'bg-slate-900 text-white' : 'border border-slate-300 bg-white text-slate-900'}"
      aria-pressed={preset === choice.id}
      onclick={() => choose(choice.id)}
    >
      {choice.label}
    </button>
  {/each}
  {#if preset === 'custom'}
    <input class="h-10 rounded-xl border border-slate-300 bg-white px-3 text-sm font-bold text-slate-900" type="date" aria-label="From" bind:value={customFrom} onchange={publish} />
    <input class="h-10 rounded-xl border border-slate-300 bg-white px-3 text-sm font-bold text-slate-900" type="date" aria-label="To" bind:value={customTo} onchange={publish} />
  {/if}
  {#if label}<span class="text-sm font-bold text-slate-500">{label}</span>{/if}
</div>
