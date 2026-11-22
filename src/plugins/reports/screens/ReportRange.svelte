<script lang="ts">
  import { onMount } from 'svelte'
  import { reportSpan, type ReportPreset, type ReportSpan } from '../../../lib/range'
  import { Button, Input } from '../../../lib/ui'

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
    <Button
      variant={preset === choice.id ? 'primary' : 'secondary'}
      size="sm"
      aria-pressed={preset === choice.id}
      onclick={() => choose(choice.id)}
    >
      {choice.label}
    </Button>
  {/each}
  {#if preset === 'custom'}
    <Input class="w-auto" type="date" aria-label="From" bind:value={customFrom} onchange={publish} />
    <Input class="w-auto" type="date" aria-label="To" bind:value={customTo} onchange={publish} />
  {/if}
  {#if label}<span class="text-sm font-bold text-slate-500">{label}</span>{/if}
</div>
