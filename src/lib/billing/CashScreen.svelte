<script lang="ts">
  import { onMount } from 'svelte'
  import { closeCashShift, loadCashDesk, openCashShift, saveCashDraft } from '../api'
  import { formatMoney } from '../money'
  import { printCashShift } from '../print'
  import type { CashDesk, CashShift } from '../types'
  import { toast, toastError } from '../toast'
  import { Button, Input, Select } from '../ui'

  interface ExpenseLine {
    description: string
    category: string
    amount: string
  }

  const expenseCategories = ['Supplies', 'Wages', 'Food', 'Transport', 'Utilities', 'Maintenance', 'Other']

  function blankExpense(): ExpenseLine {
    return { description: '', category: 'Supplies', amount: '' }
  }

  interface Props {
    currency: string
    locale: string
  }

  let { currency, locale }: Props = $props()

  let desk = $state<CashDesk | null>(null)
  let saving = $state(false)
  let printingId = $state<number | null>(null)
  let showSystem = $state(false)
  let opening = $state('')
  let petty = $state('')
  let lines = $state<ExpenseLine[]>([blankExpense()])
  let taken = $state('')
  let manual = $state('')
  let hydratedId = $state<number | null>(null)

  onMount(() => {
    refresh()
  })

  async function refresh() {
    try {
      desk = await loadCashDesk()
      if (!desk.open && opening === '') opening = String(desk.suggested_opening)
      if (desk.open && desk.open.id !== hydratedId) hydrate(desk.open)
    } catch (reason) {
      desk = null
      toastError(reason, 'The cash desk did not open.')
    }
  }

  function money(amount: number) {
    return formatMoney(amount, currency, locale)
  }

  function amount(value: string) {
    return /^\d+$/.test(value.trim()) ? Number(value) : 0
  }

  function requireAmount(value: string, label: string) {
    if (!/^\d+$/.test(value.trim())) return `${label} is a whole amount.`
    return ''
  }

  const expenseTotal = $derived(lines.reduce((sum, line) => sum + amount(line.amount), 0))
  const system = $derived(
    (desk?.open?.opening_cash ?? 0) + (desk?.open?.petty_cash ?? 0) + (desk?.open?.sales ?? 0) - expenseTotal - amount(taken),
  )
  const difference = $derived(amount(manual) - system)

  function hydrate(shift: CashShift) {
    hydratedId = shift.id
    lines = shift.lines.length
      ? [...shift.lines.map((line) => ({ description: line.description, category: line.category, amount: String(line.amount) })), blankExpense()]
      : [blankExpense()]
    taken = shift.cash_taken == null ? '' : String(shift.cash_taken)
    manual = shift.manual_count == null ? '' : String(shift.manual_count)
  }

  function formBody() {
    return {
      expenses: lines
        .filter((line) => line.description.trim() || line.amount.trim())
        .map((line) => ({
          description: line.description.trim(),
          category: line.category,
          amount: amount(line.amount),
        })),
      cash_taken: amount(taken),
      manual_count: /^\d+$/.test(manual.trim()) ? Number(manual) : null,
    }
  }

  function formProblem(closing: boolean) {
    for (const line of lines) {
      const described = line.description.trim().length > 0
      const numbered = line.amount.trim().length > 0
      if (!described && !numbered) continue
      if (!described) return 'Enter a description for each expense.'
      if (!/^\d+$/.test(line.amount.trim())) return 'Each expense amount is a whole number.'
    }
    if (taken.trim() && !/^\d+$/.test(taken.trim())) return 'Cash taken out is a whole amount.'
    if (manual.trim() && !/^\d+$/.test(manual.trim())) return 'Manual count is a whole amount.'
    if (closing && !/^\d+$/.test(manual.trim())) return 'Enter the manual count before closing the day.'
    if (closing && system < 0) return 'Cash left would be below zero. Check the expenses and the cash taken out.'
    return ''
  }

  async function startShift() {
    const problem = requireAmount(opening, 'Previous till cash') || requireAmount(petty, 'Petty cash')
    if (problem) {
      toast.error(problem)
      return
    }
    saving = true
    try {
      desk = await openCashShift(Number(opening), Number(petty))
      showSystem = false
      lines = [blankExpense()]
      taken = ''
      manual = ''
      hydratedId = desk.open?.id ?? null
      toast.success('The shift is open.')
    } catch (reason) {
      toastError(reason, 'The shift did not start.')
    } finally {
      saving = false
    }
  }

  async function saveDraft() {
    const problem = formProblem(false)
    if (problem) {
      toast.error(problem)
      return
    }
    saving = true
    try {
      desk = await saveCashDraft(formBody())
      if (desk.open) hydrate(desk.open)
      toast.success('Draft saved.')
    } catch (reason) {
      toastError(reason, 'The draft did not save.')
    } finally {
      saving = false
    }
  }

  async function finishShift() {
    const problem = formProblem(true)
    if (problem) {
      toast.error(problem)
      return
    }
    saving = true
    try {
      desk = await closeCashShift(formBody())
      toast.success('The shift is closed.')
      showSystem = false
      lines = [blankExpense()]
      taken = ''
      manual = ''
      opening = ''
      hydratedId = null
      if (!desk.open) opening = String(desk.suggested_opening)
    } catch (reason) {
      toastError(reason, 'The shift did not close.')
    } finally {
      saving = false
    }
  }

  async function printShift(id: number) {
    printingId = id
    try {
      const result = await printCashShift(id)
      if (result.printed) toast.success('The closing slip was sent to the printer.')
      else toast.warning('Printing is turned off in System settings.')
    } catch (reason) {
      toastError(reason, 'The closing slip did not print.')
    } finally {
      printingId = null
    }
  }
</script>

<div class="stock-board cash-board">
  <aside class="stock-form">
    <h2>Cash Closing</h2>
    {#if !desk}
      <p class="hint">Opening the cash desk…</p>
    {:else if !desk.open}
      <p class="hint">Start the day with the cash already in the drawer and the petty cash.</p>
      <label>
        Previous till's cash
        <Input inputmode="numeric" bind:value={opening} />
      </label>
      <label>
        Petty cash
        <Input inputmode="numeric" bind:value={petty} />
      </label>
      <Button class="mt-4 w-full" disabled={saving} onclick={startShift}>{saving ? 'Saving…' : 'Start the day'}</Button>
    {:else}
      <p class="hint">Opened {desk.open.opened_at} by {desk.open.opened_by}.</p>
      <div class="mt-3 grid grid-cols-2 gap-2">
        <p class="rounded-xl bg-slate-50 px-3 py-3 text-sm font-bold text-slate-600">Previous till <strong class="mt-1 block text-lg text-slate-900">{money(desk.open.opening_cash)}</strong></p>
        <p class="rounded-xl bg-slate-50 px-3 py-3 text-sm font-bold text-slate-600">Petty cash <strong class="mt-1 block text-lg text-slate-900">{money(desk.open.petty_cash)}</strong></p>
      </div>
      <p class="mt-3 flex items-center justify-between text-sm font-extrabold text-slate-900">
        <span>Cash sales this shift</span>
        <span>{money(desk.open.sales)}</span>
      </p>
      <p class="stock-label">Expenses</p>
      {#each lines as line, index (index)}
        <div class="mt-2 rounded-xl border border-slate-200 p-3">
          <label class="mt-0">
            Description
            <Input bind:value={line.description} placeholder="Tea for staff" />
          </label>
          <div class="stock-pair">
            <label>
              Category
              <Select bind:value={line.category}>
                {#each expenseCategories as category (category)}
                  <option value={category}>{category}</option>
                {/each}
              </Select>
            </label>
            <label>
              Amount
              <Input inputmode="numeric" bind:value={line.amount} />
            </label>
          </div>
          <Button variant="link" class="text-danger" onclick={() => (lines = lines.filter((_, item) => item !== index).concat(lines.length === 1 ? [blankExpense()] : []))}>Remove</Button>
        </div>
      {/each}
      <Button variant="secondary" class="mt-3" onclick={() => (lines = [...lines, blankExpense()])}>Add expense</Button>
      <p class="mt-3 flex items-center justify-between text-sm font-extrabold text-slate-900">
        <span>Expense total</span>
        <span>{money(expenseTotal)}</span>
      </p>
      <label>
        Cash taken out
        <Input inputmode="numeric" bind:value={taken} />
      </label>
      <label>
        Manual count
        <Input inputmode="numeric" bind:value={manual} />
      </label>
      <Button variant="secondary" class="mt-3" onclick={() => (showSystem = !showSystem)}>
        {showSystem ? 'Hide system count' : 'Show system count'}
      </Button>
      {#if showSystem}
        <div class="mt-3 rounded-xl bg-slate-50 px-3 py-3">
          <p class="hint mt-0">Opening + petty + sales − expenses − cash taken out.</p>
          <p class="mt-2 flex items-center justify-between text-sm font-extrabold text-slate-900"><span>System count</span><span>{money(system)}</span></p>
          {#if /^\d+$/.test(manual.trim())}
            <p class="mt-1 flex items-center justify-between text-sm font-extrabold {difference === 0 ? 'text-emerald-700' : 'text-rose-700'}"><span>Difference</span><span>{money(difference)}</span></p>
          {/if}
        </div>
      {/if}
      <div class="mt-3 rounded-xl bg-slate-50 px-3 py-3">
        <p class="hint mt-0">Previous till + petty + sales − expenses − cash taken out.</p>
        <p class="mt-2 flex items-center justify-between text-sm font-extrabold {system < 0 ? 'text-rose-700' : 'text-slate-900'}"><span>Cash left for the next shift</span><span>{money(system)}</span></p>
      </div>
      <Button variant="secondary" class="mt-3 w-full" disabled={saving} onclick={saveDraft}>Save draft</Button>
      <Button class="mt-3 w-full" disabled={saving} onclick={finishShift}>{saving ? 'Saving…' : 'Close the day'}</Button>
    {/if}
  </aside>

  <section class="stock-summary">
    <div class="summary-head">
      <div>
        <h2>Closed shifts</h2>
        <p class="hint">Each closing keeps the count that was saved</p>
      </div>
    </div>
    <div class="stock-table-wrap">
      {#if desk && desk.history.length === 0}
        <p class="hint">No shift has been closed yet.</p>
      {:else if desk}
        {#each desk.history as shift (shift.id)}
          <article class="ledger-row">
            <div>
              <strong>{shift.closed_at}</strong>
              <p>{shift.closed_by ?? shift.opened_by}</p>
              <div class="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-[12px] font-bold text-slate-600">
                <span>Sales {money(shift.sales)}</span>
                <span>Expenses {money(shift.expenses ?? 0)}</span>
                <span>Manual {money(shift.manual_count ?? 0)}</span>
                <span>Left {money(shift.cash_left ?? 0)}</span>
                <span class={shift.difference === 0 ? 'text-emerald-700' : 'text-rose-700'}>Difference {money(shift.difference ?? 0)}</span>
              </div>
            </div>
            <Button variant="secondary" size="sm" disabled={printingId === shift.id} onclick={() => printShift(shift.id)}>
              {printingId === shift.id ? 'Printing…' : 'Print'}
            </Button>
          </article>
        {/each}
      {/if}
    </div>
  </section>
</div>
