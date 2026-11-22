<script lang="ts">
  import { onMount } from 'svelte'
  import { request } from '../../../lib/api'
  import { formatMoney } from '../../../lib/money'
  import { toast, toastError, toastSaved } from '../../../lib/toast'
  import { Button, Field, Input, Segmented, Select, Textarea } from '../../../lib/ui'

  interface Props {
    currency: string
    locale: string
  }

  let { currency, locale }: Props = $props()

  interface JobSummary {
    id: number
    customer_name: string
    garment: string
    delivery_date: string | null
    status: string
    delivery_status: string
    total_amount: number
    paid_amount: number
    balance: number
  }

  interface Payment {
    id: number
    amount: number
    method: string
    note: string
    paid_on: string
  }

  interface Job extends JobSummary {
    phone: string
    measurements: Record<string, string>
    cloth: { type: string; color: string; quantity: string; source: string; notes: string }
    sewing_details: string
    notes: string
    payments: Payment[]
  }

  const jobStatuses = [
    ['received', 'Received'],
    ['cutting', 'Cutting'],
    ['sewing', 'Sewing'],
    ['finishing', 'Finishing'],
    ['ready', 'Ready'],
    ['cancelled', 'Cancelled'],
  ] as const

  const deliveryStatuses = [
    ['pending', 'Not sent'],
    ['ready', 'Ready to deliver'],
    ['out', 'Out for delivery'],
    ['delivered', 'Delivered'],
  ] as const

  const measures = [
    ['chest', 'Chest'],
    ['waist', 'Waist'],
    ['hip', 'Hip'],
    ['shoulder', 'Shoulder'],
    ['sleeve', 'Sleeve'],
    ['length', 'Length'],
    ['neck', 'Neck'],
    ['thigh', 'Thigh'],
    ['inseam', 'Inseam'],
  ] as const

  const paymentMethods = [
    ['cash', 'Cash'],
    ['card', 'Card'],
    ['transfer', 'Transfer'],
    ['credit', 'Credit'],
  ] as const

  const clothSources = [
    { value: 'customer', label: 'Customer' },
    { value: 'shop', label: 'Shop' },
  ]

  const statusClasses: Record<string, string> = {
    received: 'bg-slate-100 text-slate-600',
    cutting: 'bg-sky-100 text-sky-700',
    sewing: 'bg-violet-100 text-violet-700',
    finishing: 'bg-amber-100 text-amber-700',
    ready: 'bg-emerald-100 text-emerald-700',
    cancelled: 'bg-rose-100 text-rose-700',
  }

  const deliveryClasses: Record<string, string> = {
    pending: 'bg-slate-100 text-slate-600',
    ready: 'bg-amber-100 text-amber-700',
    out: 'bg-sky-100 text-sky-700',
    delivered: 'bg-emerald-100 text-emerald-700',
  }

  const statusAccents: Record<string, string> = {
    received: 'border-t-slate-400',
    cutting: 'border-t-sky-500',
    sewing: 'border-t-violet-500',
    finishing: 'border-t-amber-500',
    ready: 'border-t-emerald-500',
    cancelled: 'border-t-rose-500',
  }

  const avatarTones = ['#0f766e', '#7c3aed', '#2563eb', '#b45309', '#be123c', '#4d7c0f']

  let jobs = $state<JobSummary[]>([])
  let selected = $state<Job | null>(null)
  let creating = $state(false)
  let saving = $state(false)
  let loading = $state(true)
  let failed = $state(false)
  let query = $state('')
  let statusFilter = $state('all')
  let nameInput = $state<HTMLInputElement | null>(null)

  let customerName = $state('')
  let phone = $state('')
  let garment = $state('')
  let measurement = $state<Record<string, string>>({})
  let otherMeasure = $state('')
  let clothType = $state('')
  let clothColor = $state('')
  let clothQuantity = $state('')
  let clothSource = $state('customer')
  let clothNotes = $state('')
  let sewing = $state('')
  let notes = $state('')
  let deliveryDate = $state('')
  let total = $state('')
  let advance = $state('')
  let advanceMethod = $state('cash')

  let payAmount = $state('')
  let payMethod = $state('cash')
  let payNote = $state('')

  function label(options: readonly (readonly [string, string])[], value: string) {
    return options.find((option) => option[0] === value)?.[1] ?? value
  }

  function money(amount: number) {
    return formatMoney(amount, currency, locale)
  }

  function initials(name: string) {
    const words = name.trim().split(/\s+/).filter(Boolean)
    const letters = words.length >= 2 ? `${words[0][0]}${words[1][0]}` : name.slice(0, 2)
    return letters.toUpperCase() || 'JB'
  }

  function toneColor(name: string) {
    let hash = 0
    for (const character of name) hash = (hash * 31 + character.charCodeAt(0)) >>> 0
    return avatarTones[hash % avatarTones.length]
  }

  function toneGradient(name: string) {
    const tone = toneColor(name)
    return `linear-gradient(135deg, color-mix(in srgb, ${tone} 88%, black), ${tone})`
  }

  function startOfToday() {
    const date = new Date()
    date.setHours(0, 0, 0, 0)
    return date
  }

  function dayDiff(value: string | null) {
    if (!value) return null
    const target = new Date(`${value}T00:00:00`)
    if (Number.isNaN(target.getTime())) return null
    return Math.round((target.getTime() - startOfToday().getTime()) / 86_400_000)
  }

  function dateLabel(value: string | null) {
    if (!value) return 'No date'
    const date = value.length === 10 ? new Date(`${value}T00:00:00`) : new Date(value)
    if (Number.isNaN(date.getTime())) return value
    return date.toLocaleDateString(locale, { day: 'numeric', month: 'short', year: 'numeric' })
  }

  function dueText(job: JobSummary) {
    if (!job.delivery_date) return 'No delivery date'
    const diff = dayDiff(job.delivery_date)
    if (job.delivery_status === 'delivered') return `${dateLabel(job.delivery_date)} · delivered`
    if (job.status === 'cancelled') return dateLabel(job.delivery_date)
    if (diff === null) return dateLabel(job.delivery_date)
    if (diff < 0) return `${dateLabel(job.delivery_date)} · ${Math.abs(diff)}d late`
    if (diff === 0) return `${dateLabel(job.delivery_date)} · today`
    if (diff === 1) return `${dateLabel(job.delivery_date)} · tomorrow`
    return dateLabel(job.delivery_date)
  }

  function urgency(job: JobSummary) {
    if (job.status === 'cancelled') return 'Order cancelled'
    if (job.delivery_status === 'delivered') return 'Delivered to the customer'
    if (!job.delivery_date) return 'No delivery date set'
    const diff = dayDiff(job.delivery_date)
    if (diff === null) return dateLabel(job.delivery_date)
    if (diff < 0) {
      const days = Math.abs(diff)
      return `Overdue by ${days} ${days === 1 ? 'day' : 'days'}`
    }
    if (diff === 0) return 'Due today'
    if (diff === 1) return 'Due tomorrow'
    return `Due in ${diff} days`
  }

  function isLate(job: JobSummary) {
    if (job.status === 'cancelled' || job.delivery_status === 'delivered') return false
    const diff = dayDiff(job.delivery_date)
    return diff !== null && diff < 0
  }

  function isDueToday(job: JobSummary) {
    if (job.status === 'cancelled' || job.delivery_status === 'delivered') return false
    return dayDiff(job.delivery_date) === 0
  }

  function paidPercent(job: JobSummary) {
    if (job.total_amount <= 0) return job.balance <= 0 ? 100 : 0
    return Math.max(0, Math.min(100, Math.round((job.paid_amount / job.total_amount) * 100)))
  }

  function statusClass(value: string) {
    return statusClasses[value] ?? statusClasses.received
  }

  function deliveryClass(value: string) {
    return deliveryClasses[value] ?? deliveryClasses.pending
  }

  function cardAccent(job: JobSummary) {
    if (isLate(job)) return 'border-t-rose-600'
    return statusAccents[job.status] ?? 'border-t-teal-600'
  }

  function metaClass(job: JobSummary) {
    if (isLate(job)) return 'border-rose-200 bg-rose-50 text-rose-700'
    if (isDueToday(job)) return 'border-amber-200 bg-amber-50 text-amber-700'
    return 'border-slate-100 bg-slate-50 text-slate-500'
  }

  function filterClass(active: boolean) {
    return active ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:bg-white/60'
  }

  function countClass(active: boolean) {
    return active ? 'bg-teal-700 text-white' : 'bg-slate-900/5'
  }

  /** Stagger for list entrances so cards settle in one after another. */
  function stagger(index: number) {
    return { animationDelay: `${Math.min(index, 10) * 28}ms` }
  }

  const counts = $derived.by(() => {
    let open = 0
    let working = 0
    let due = 0
    let balance = 0
    for (const job of jobs) {
      const cancelled = job.status === 'cancelled'
      const delivered = job.delivery_status === 'delivered'
      if (!cancelled && !delivered) open += 1
      if (job.status === 'cutting' || job.status === 'sewing' || job.status === 'finishing') working += 1
      if (!cancelled && !delivered) {
        const diff = dayDiff(job.delivery_date)
        if (diff !== null && diff <= 0) due += 1
      }
      if (!cancelled) balance += Math.max(0, job.balance)
    }
    return { open, working, due, balance }
  })

  const statusCounts = $derived.by(() => {
    const map: Record<string, number> = { all: jobs.length }
    for (const job of jobs) map[job.status] = (map[job.status] ?? 0) + 1
    return map
  })

  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase()
    const rank = (job: JobSummary) => {
      if (job.status === 'cancelled') return 2
      if (job.delivery_status === 'delivered') return 1
      return 0
    }
    return jobs
      .filter((job) => {
        if (statusFilter !== 'all' && job.status !== statusFilter) return false
        if (!needle) return true
        return job.customer_name.toLowerCase().includes(needle) || job.garment.toLowerCase().includes(needle)
      })
      .sort((a, b) => {
        const byRank = rank(a) - rank(b)
        if (byRank !== 0) return byRank
        const aDate = a.delivery_date ?? '9999-12-31'
        const bDate = b.delivery_date ?? '9999-12-31'
        if (aDate !== bDate) return aDate.localeCompare(bDate)
        return b.id - a.id
      })
  })

  const measured = $derived.by(() => {
    const job = selected
    if (!job) return [] as Array<readonly [string, string]>
    return measures.flatMap(([key, name]) => {
      const value = job.measurements[key]?.trim()
      return value ? [[name, value] as const] : []
    })
  })

  const preview = $derived.by(() => {
    if (!/^\d+$/.test(total.trim())) return null
    const totalValue = Number(total.trim())
    const advanceValue = /^\d+$/.test(advance.trim()) ? Number(advance.trim()) : 0
    return {
      total: totalValue,
      advance: advanceValue,
      balance: Math.max(0, totalValue - advanceValue),
    }
  })

  async function load(showSkeleton = jobs.length === 0) {
    if (showSkeleton) loading = true
    failed = false
    try {
      const body = await request<{ jobs: JobSummary[] }>('/api/v1/tailor/jobs')
      jobs = body.jobs
    } catch (error) {
      failed = true
      toastError(error, 'Could not load jobs.')
    } finally {
      loading = false
    }
  }

  async function openJob(id: number) {
    creating = false
    try {
      selected = await request<Job>(`/api/v1/tailor/jobs/${id}`)
    } catch (error) {
      toastError(error, 'Could not open that job.')
    }
  }

  function closeJob() {
    selected = null
  }

  function openCreate() {
    selected = null
    creating = true
  }

  function closeCreate() {
    creating = false
    resetForm()
  }

  function resetForm() {
    customerName = ''
    phone = ''
    garment = ''
    measurement = {}
    otherMeasure = ''
    clothType = ''
    clothColor = ''
    clothQuantity = ''
    clothSource = 'customer'
    clothNotes = ''
    sewing = ''
    notes = ''
    deliveryDate = ''
    total = ''
    advance = ''
    advanceMethod = 'cash'
  }

  async function createOrder() {
    const amount = Number(total)
    if (!customerName.trim()) {
      toast.error('Enter the customer name.')
      return
    }
    if (!garment.trim()) {
      toast.error('Enter what is being sewn.')
      return
    }
    if (!Number.isInteger(amount) || amount < 0) {
      toast.error('Enter the total as a whole amount.')
      return
    }
    const payments = []
    if (advance.trim()) {
      const paid = Number(advance)
      if (!Number.isInteger(paid) || paid < 1) {
        toast.error('Enter the advance as a whole amount.')
        return
      }
      if (paid > amount) {
        toast.error('The advance is more than the total.')
        return
      }
      payments.push({ amount: paid, method: advanceMethod, note: 'Advance' })
    }
    saving = true
    try {
      const job = await request<Job>('/api/v1/tailor/jobs', {
        method: 'POST',
        body: JSON.stringify({
          customer_name: customerName.trim(),
          phone: phone.trim(),
          garment: garment.trim(),
          measurements: { ...measurement, other: otherMeasure.trim() },
          cloth: {
            type: clothType.trim(),
            color: clothColor.trim(),
            quantity: clothQuantity.trim(),
            source: clothSource,
            notes: clothNotes.trim(),
          },
          sewing_details: sewing.trim(),
          notes: notes.trim(),
          delivery_date: deliveryDate,
          total_amount: amount,
          payments,
        }),
      })
      selected = job
      creating = false
      resetForm()
      await load(false)
      toastSaved('Order saved.')
    } catch (error) {
      toastError(error, 'Could not save the order.')
    } finally {
      saving = false
    }
  }

  async function saveStatus() {
    if (!selected) return
    saving = true
    try {
      selected = await request<Job>(`/api/v1/tailor/jobs/${selected.id}`, {
        method: 'PUT',
        body: JSON.stringify({ status: selected.status, delivery_status: selected.delivery_status }),
      })
      await load(false)
      toastSaved('Job updated.')
    } catch (error) {
      toastError(error, 'Could not update the job.')
    } finally {
      saving = false
    }
  }

  async function addPayment() {
    if (!selected) return
    const amount = Number(payAmount)
    if (!Number.isInteger(amount) || amount < 1) {
      toast.error('Enter a payment above 0.')
      return
    }
    saving = true
    try {
      selected = await request<Job>(`/api/v1/tailor/jobs/${selected.id}/payments`, {
        method: 'POST',
        body: JSON.stringify({ amount, method: payMethod, note: payNote.trim() }),
      })
      payAmount = ''
      payNote = ''
      await load(false)
      toastSaved('Payment saved.')
    } catch (error) {
      toastError(error, 'Could not save the payment.')
    } finally {
      saving = false
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return
    if (creating) closeCreate()
    else if (selected) closeJob()
  }

  $effect(() => {
    if (creating) nameInput?.focus()
  })

  onMount(() => {
    load()
  })
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet pill(text: string, classes: string)}
  <span class="inline-flex h-[26px] items-center gap-1.5 whitespace-nowrap rounded-full px-2.5 text-[11px] font-extrabold {classes}">
    <span class="h-1.5 w-1.5 rounded-full bg-current opacity-80"></span>
    {text}
  </span>
{/snippet}

<section class="flex w-full min-w-0 flex-1 flex-col gap-4 pb-5">
  <header class="flex flex-wrap items-end justify-between gap-6">
    <div class="min-w-0">
      <span class="flex items-center gap-2 text-[11px] font-extrabold uppercase tracking-[0.09em] text-teal-700">
        <span class="h-0.5 w-5 rounded-full bg-current"></span>
        Tailor
      </span>
      <h1 class="mt-1.5 text-3xl font-extrabold tracking-tight text-ink">Jobs</h1>
      <p class="mt-2 text-[13px] font-semibold text-slate-500">
        Orders, measurements, cloth, delivery and payments — all in one place.
      </p>
    </div>
    <div class="flex flex-wrap gap-2.5 max-sm:w-full">
      <Button class="max-sm:flex-1" variant="secondary" onclick={() => load(true)} disabled={loading}>
        <svg class="h-4 w-4 fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 5v6h-6" />
        </svg>
        Refresh
      </Button>
      <Button class="max-sm:flex-1" onclick={openCreate}>
        <svg class="h-4 w-4 fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M12 5v14M5 12h14" />
        </svg>
        New order
      </Button>
    </div>
  </header>

  <section class="grid grid-cols-[repeat(auto-fit,minmax(190px,1fr))] gap-3" aria-label="Job overview">
    <article class="flex items-center gap-3.5 rounded-[18px] border border-slate-200 bg-white px-4 py-3.5 shadow-[0_8px_24px_rgba(15,23,42,0.04)]">
      <span class="grid h-[42px] w-[42px] flex-none place-items-center rounded-[13px] bg-emerald-100 text-emerald-700" aria-hidden="true">
        <svg class="h-5 w-5 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="6" cy="6" r="2.5" /><circle cx="6" cy="18" r="2.5" /><path d="M8 8l12 10M8 16L20 6" />
        </svg>
      </span>
      <div>
        <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Active orders</span>
        <strong class="mt-0.5 block text-2xl leading-none tracking-tight text-ink">{counts.open}</strong>
        <p class="mt-1 text-[11px] font-bold text-slate-400">{counts.open === 1 ? 'order in hand' : 'orders in hand'}</p>
      </div>
    </article>

    <article class="flex items-center gap-3.5 rounded-[18px] border border-slate-200 bg-white px-4 py-3.5 shadow-[0_8px_24px_rgba(15,23,42,0.04)]">
      <span class="grid h-[42px] w-[42px] flex-none place-items-center rounded-[13px] bg-violet-100 text-violet-700" aria-hidden="true">
        <svg class="h-5 w-5 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 20l7-7" /><path d="M14 4l6 6-7 7-6-6z" />
        </svg>
      </span>
      <div>
        <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">In the works</span>
        <strong class="mt-0.5 block text-2xl leading-none tracking-tight text-ink">{counts.working}</strong>
        <p class="mt-1 text-[11px] font-bold text-slate-400">cutting, sewing or finishing</p>
      </div>
    </article>

    <article
      class="flex items-center gap-3.5 rounded-[18px] border bg-white px-4 py-3.5 {counts.due > 0
        ? 'border-rose-200 shadow-[0_8px_24px_rgba(190,18,60,0.07)]'
        : 'border-slate-200 shadow-[0_8px_24px_rgba(15,23,42,0.04)]'}"
    >
      <span class="grid h-[42px] w-[42px] flex-none place-items-center rounded-[13px] bg-amber-100 text-amber-700" aria-hidden="true">
        <svg class="h-5 w-5 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3l9 16H3z" /><path d="M12 9v4M12 16h.01" />
        </svg>
      </span>
      <div>
        <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Due or late</span>
        <strong class="mt-0.5 block text-2xl leading-none tracking-tight text-ink">{counts.due}</strong>
        <p class="mt-1 text-[11px] font-bold text-slate-400">{counts.due === 0 ? 'nothing pressing' : 'needs attention'}</p>
      </div>
    </article>

    <article class="flex items-center gap-3.5 rounded-[18px] border border-slate-200 bg-white px-4 py-3.5 shadow-[0_8px_24px_rgba(15,23,42,0.04)]">
      <span class="grid h-[42px] w-[42px] flex-none place-items-center rounded-[13px] bg-blue-100 text-blue-700" aria-hidden="true">
        <svg class="h-5 w-5 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="6" width="18" height="12" rx="3" /><circle cx="12" cy="12" r="2.5" />
        </svg>
      </span>
      <div>
        <span class="block text-[10px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Outstanding</span>
        <strong class="mt-0.5 block text-2xl leading-none tracking-tight text-ink">{money(counts.balance)}</strong>
        <p class="mt-1 text-[11px] font-bold text-slate-400">left to collect</p>
      </div>
    </article>
  </section>

  <section class="flex min-w-0 flex-wrap items-center gap-3">
    <div class="relative flex min-w-[220px] max-w-[420px] flex-1 items-center">
      <svg class="pointer-events-none absolute left-3.5 h-[18px] w-[18px] fill-none stroke-current stroke-[1.8] text-slate-400" viewBox="0 0 24 24" stroke-linecap="round" aria-hidden="true">
        <circle cx="11" cy="11" r="6.5" /><path d="M16 16l4.5 4.5" />
      </svg>
      <Input class="pl-10 pr-10" bind:value={query} placeholder="Search customer or garment" aria-label="Search jobs" />
      {#if query}
        <button
          class="absolute right-2 grid h-7 w-7 place-items-center rounded-lg bg-slate-100 text-lg leading-none text-slate-500 transition-colors hover:bg-slate-200 hover:text-slate-900"
          type="button"
          onclick={() => (query = '')}
          aria-label="Clear search"
        >×</button>
      {/if}
    </div>

    <div class="flex min-w-0 basis-full gap-1 overflow-x-auto rounded-xl bg-slate-200/70 p-1 md:basis-auto" role="group" aria-label="Filter by job status">
      <button
        class="inline-flex h-9 items-center gap-1.5 whitespace-nowrap rounded-lg px-3 text-xs font-extrabold transition-colors {filterClass(statusFilter === 'all')}"
        aria-pressed={statusFilter === 'all'}
        onclick={() => (statusFilter = 'all')}
      >
        All
        <span class="grid h-5 min-w-5 place-items-center rounded-full px-1.5 text-[10px] {countClass(statusFilter === 'all')}">
          {statusCounts.all ?? 0}
        </span>
      </button>
      {#each jobStatuses as [value, name] (value)}
        <button
          class="inline-flex h-9 items-center gap-1.5 whitespace-nowrap rounded-lg px-3 text-xs font-extrabold transition-colors {filterClass(statusFilter === value)}"
          aria-pressed={statusFilter === value}
          onclick={() => (statusFilter = value)}
        >
          {name}
          <span class="grid h-5 min-w-5 place-items-center rounded-full px-1.5 text-[10px] {countClass(statusFilter === value)}">
            {statusCounts[value] ?? 0}
          </span>
        </button>
      {/each}
    </div>
  </section>

  {#if failed}
    <section class="flex flex-col items-center gap-2.5 rounded-[20px] border border-dashed border-rose-200 bg-rose-50/50 px-6 py-14 text-center">
      <span class="grid h-14 w-14 place-items-center rounded-2xl bg-rose-100 text-rose-700" aria-hidden="true">
        <svg class="h-[26px] w-[26px] fill-none stroke-current stroke-[1.6]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 8v5M12 17h.01" /><circle cx="12" cy="12" r="9" />
        </svg>
      </span>
      <h2 class="mt-0.5 text-lg font-extrabold tracking-tight text-ink">The job list did not open</h2>
      <p class="max-w-[390px] text-[13px] font-semibold text-slate-500">Check the till connection, then try again.</p>
      <Button variant="secondary" onclick={() => load(true)}>Try again</Button>
    </section>
  {:else if loading}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] items-start gap-3.5 max-[420px]:grid-cols-1" aria-hidden="true">
      {#each [0, 1, 2, 3] as item (item)}
        <div class="flex flex-col gap-3 rounded-2xl border border-slate-200 border-t-[3px] border-t-slate-200 bg-white p-4">
          <div class="flex items-center gap-3">
            <span class="job-skeleton h-[42px] w-[42px] flex-none rounded-xl"></span>
            <div class="flex flex-1 flex-col gap-2">
              <span class="job-skeleton h-2.5 w-[70%] rounded-lg"></span>
              <span class="job-skeleton h-2.5 w-[40%] rounded-lg"></span>
            </div>
          </div>
          <span class="job-skeleton h-2.5 w-full rounded-lg"></span>
          <span class="job-skeleton h-2.5 w-[80%] rounded-lg"></span>
          <span class="job-skeleton mt-1.5 h-1.5 rounded-lg"></span>
        </div>
      {/each}
    </div>
  {:else if filtered.length === 0}
    <section class="flex flex-col items-center gap-2.5 rounded-[20px] border border-dashed border-slate-300 bg-white px-6 py-14 text-center">
      <span class="grid h-14 w-14 place-items-center rounded-2xl bg-emerald-50 text-teal-700" aria-hidden="true">
        <svg class="h-[26px] w-[26px] fill-none stroke-current stroke-[1.6]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="6" cy="6" r="2.5" /><circle cx="6" cy="18" r="2.5" /><path d="M8 8l12 10M8 16L20 6" />
        </svg>
      </span>
      <h2 class="mt-0.5 text-lg font-extrabold tracking-tight text-ink">{jobs.length === 0 ? 'No orders yet' : 'No jobs match'}</h2>
      <p class="max-w-[390px] text-[13px] font-semibold text-slate-500">
        {jobs.length === 0
          ? 'Take the first order to start tracking measurements, cloth and payments.'
          : 'Try another status or clear the search to see the whole board.'}
      </p>
      {#if jobs.length === 0}
        <Button onclick={openCreate}>Create the first order</Button>
      {:else}
        <Button variant="secondary" onclick={() => { query = ''; statusFilter = 'all' }}>Clear filters</Button>
      {/if}
    </section>
  {:else}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] items-start gap-3.5 max-[420px]:grid-cols-1">
      {#each filtered as job, index (job.id)}
        <button
          type="button"
          style={stagger(index)}
          class="job-enter flex w-full cursor-pointer flex-col gap-3 rounded-2xl border border-slate-200 border-t-[3px] bg-white p-4 text-left text-ink shadow-sm transition-all duration-200 ease-out hover:-translate-y-1 hover:shadow-lg focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-focus/30 active:translate-y-0 active:scale-[0.995] {cardAccent(job)}"
          onclick={() => openJob(job.id)}
        >
          <div class="flex min-w-0 items-center gap-3">
            <span class="job-pop grid h-[42px] w-[42px] flex-none place-items-center rounded-xl text-sm font-extrabold text-white shadow-sm" style:background={toneGradient(job.customer_name)}>{initials(job.customer_name)}</span>
            <div class="flex min-w-0 flex-1 flex-col gap-0.5">
              <strong class="truncate text-[15px] font-bold">{job.customer_name}</strong>
              <span class="truncate text-xs font-semibold text-slate-500">{job.garment}</span>
            </div>
            {@render pill(label(jobStatuses, job.status), statusClass(job.status))}
          </div>

          <div class="flex flex-wrap items-center gap-2">
            <span class="inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border px-2.5 py-1 text-[11px] font-bold {metaClass(job)}">
              <svg class="h-3.5 w-3.5 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect x="3" y="4.5" width="18" height="16" rx="3" /><path d="M8 2.5v4M16 2.5v4M3 9.5h18" />
              </svg>
              {dueText(job)}
            </span>
            <span class="inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border border-slate-100 bg-slate-50 px-2.5 py-1 text-[11px] font-bold text-slate-500">
              <svg class="h-3.5 w-3.5 fill-none stroke-current stroke-[1.7]" viewBox="0 0 24 24" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M3 6h11v9H3zM14 9h4l3 3v3h-7z" /><circle cx="7" cy="18" r="1.6" /><circle cx="17" cy="18" r="1.6" />
              </svg>
              {label(deliveryStatuses, job.delivery_status)}
            </span>
          </div>

          <div class="mt-auto flex flex-col gap-1.5 pt-1">
            <div class="flex items-baseline justify-between gap-2">
              <span class="text-[11px] font-bold text-slate-500">Total {money(job.total_amount)}</span>
              <strong class="whitespace-nowrap text-base font-extrabold tracking-tight {job.balance > 0 ? 'text-rose-700' : 'text-ink'}">
                {money(job.balance)} <small class="text-[10px] font-bold tracking-normal text-slate-400">{job.balance > 0 ? 'left' : 'settled'}</small>
              </strong>
            </div>
            <div class="h-1.5 overflow-hidden rounded-full bg-slate-200">
              <i class="job-bar block h-full rounded-full bg-linear-to-r from-teal-600 to-emerald-500" style:width={`${paidPercent(job)}%`}></i>
            </div>
            <div class="flex justify-between gap-2 text-[10px] font-bold text-slate-400">
              <span>Paid {money(job.paid_amount)}</span>
              <span>{paidPercent(job)}% paid</span>
            </div>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</section>

{#if selected}
  <div class="fixed inset-0 z-50 flex justify-end">
    <button class="job-fade absolute inset-0 cursor-pointer bg-slate-900/40 backdrop-blur-sm" type="button" aria-label="Close job details" onclick={closeJob}></button>
    <div class="job-drawer relative z-10 flex h-dvh w-full max-w-[500px] flex-col bg-slate-50 shadow-2xl" role="dialog" aria-modal="true" aria-label={`${selected.customer_name} job`}>
      <header class="flex items-center gap-3.5 border-b-4 border-teal-600 bg-slate-900 p-5 text-white">
        <span class="grid h-14 w-14 flex-none place-items-center rounded-2xl bg-white/15 text-lg font-extrabold text-white">
          {initials(selected.customer_name)}
        </span>
        <div class="min-w-0 flex-1">
          <h2 class="text-xl font-extrabold tracking-tight">{selected.customer_name}</h2>
          <p class="mt-0.5 text-xs font-semibold text-white/75">{selected.garment}{selected.phone ? ` · ${selected.phone}` : ''}</p>
        </div>
        <button
          class="grid h-9 w-9 flex-none place-items-center rounded-xl bg-white/10 text-white transition-colors hover:bg-white/20"
          type="button"
          onclick={closeJob}
          aria-label="Close"
        >
          <svg class="h-[18px] w-[18px] fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </header>

      <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-5 pb-8 pt-4">
        <div class="flex flex-wrap gap-2">
          {@render pill(label(jobStatuses, selected.status), statusClass(selected.status))}
          {@render pill(label(deliveryStatuses, selected.delivery_status), deliveryClass(selected.delivery_status))}
          {#if isLate(selected)}
            {@render pill('Overdue', 'bg-rose-100 text-rose-700')}
          {/if}
        </div>

        <div class="grid grid-cols-3 gap-2.5">
          <div class="rounded-xl border border-slate-200 bg-white px-3 py-2.5">
            <span class="block text-[10px] font-extrabold uppercase tracking-[0.05em] text-slate-500">Total</span>
            <strong class="mt-0.5 block text-base tracking-tight text-ink">{money(selected.total_amount)}</strong>
          </div>
          <div class="rounded-xl border border-slate-200 bg-white px-3 py-2.5">
            <span class="block text-[10px] font-extrabold uppercase tracking-[0.05em] text-slate-500">Paid</span>
            <strong class="mt-0.5 block text-base tracking-tight text-emerald-700">{money(selected.paid_amount)}</strong>
          </div>
          <div class="rounded-xl border border-slate-200 bg-white px-3 py-2.5">
            <span class="block text-[10px] font-extrabold uppercase tracking-[0.05em] text-slate-500">Balance</span>
            <strong class="mt-0.5 block text-base tracking-tight {selected.balance > 0 ? 'text-rose-700' : 'text-ink'}">{money(selected.balance)}</strong>
          </div>
        </div>

        <div class="h-2 overflow-hidden rounded-full bg-slate-200">
          <i class="job-bar block h-full rounded-full bg-linear-to-r from-teal-600 to-emerald-500" style:width={`${paidPercent(selected)}%`}></i>
        </div>
        <p class="-mt-1.5 text-[11px] font-bold text-slate-400">{paidPercent(selected)}% paid · {urgency(selected)}</p>

        <section class="flex flex-col gap-3 rounded-2xl border border-slate-200 bg-white p-4">
          <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Measurements</h3>
          {#if measured.length > 0}
            <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
              {#each measured as [name, value] (name)}
                <span class="flex flex-col gap-0.5 rounded-xl border border-slate-100 bg-slate-50 px-2.5 py-2 text-[13px] font-bold text-slate-900">
                  <b class="text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">{name}</b>
                  {value}
                </span>
              {/each}
            </div>
          {:else}
            <p class="text-xs font-semibold text-slate-400">No measurements saved.</p>
          {/if}
          {#if selected.measurements.other?.trim()}
            <p class="text-xs font-semibold text-slate-500">Other · {selected.measurements.other}</p>
          {/if}
        </section>

        <section class="flex flex-col gap-3 rounded-2xl border border-slate-200 bg-white p-4">
          <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Cloth</h3>
          <div class="grid grid-cols-2 gap-2">
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Type</span>
              <strong class="mt-0.5 block text-[13px] text-slate-900">{selected.cloth.type || '—'}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Color</span>
              <strong class="mt-0.5 block text-[13px] text-slate-900">{selected.cloth.color || '—'}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Quantity</span>
              <strong class="mt-0.5 block text-[13px] text-slate-900">{selected.cloth.quantity || '—'}</strong>
            </div>
            <div class="rounded-xl bg-slate-50 px-2.5 py-2">
              <span class="block text-[9px] font-extrabold uppercase tracking-[0.06em] text-slate-400">Provided by</span>
              <strong class="mt-0.5 block text-[13px] text-slate-900">{selected.cloth.source === 'shop' ? 'Shop' : 'Customer'}</strong>
            </div>
          </div>
          {#if selected.cloth.notes.trim()}
            <p class="text-xs font-semibold text-slate-500">{selected.cloth.notes}</p>
          {/if}
        </section>

        {#if selected.sewing_details.trim() || selected.notes.trim()}
          <section class="flex flex-col gap-3 rounded-2xl border border-slate-200 bg-white p-4">
            <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Work notes</h3>
            {#if selected.sewing_details.trim()}
              <p class="text-[13px] font-semibold leading-relaxed text-slate-700">{selected.sewing_details}</p>
            {/if}
            {#if selected.notes.trim()}
              <p class="text-xs font-semibold text-slate-500">{selected.notes}</p>
            {/if}
          </section>
        {/if}

        <section class="flex flex-col gap-3 rounded-2xl border border-slate-200 bg-white p-4">
          <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Update status</h3>
          <div class="grid gap-2.5 sm:grid-cols-2 [&_.field]:mt-0">
            <Field label="Job status">
              <Select bind:value={selected.status}>
                {#each jobStatuses as [value, name] (value)}
                  <option {value}>{name}</option>
                {/each}
              </Select>
            </Field>
            <Field label="Delivery">
              <Select bind:value={selected.delivery_status}>
                {#each deliveryStatuses as [value, name] (value)}
                  <option {value}>{name}</option>
                {/each}
              </Select>
            </Field>
          </div>
          <div class="flex flex-wrap justify-end gap-2">
            <Button size="sm" disabled={saving} onclick={saveStatus}>{saving ? 'Saving…' : 'Save status'}</Button>
          </div>
        </section>

        <section class="flex flex-col gap-3 rounded-2xl border border-slate-200 bg-white p-4">
          <h3 class="text-[11px] font-extrabold uppercase tracking-[0.06em] text-slate-500">Payments</h3>
          {#if selected.payments.length === 0}
            <p class="text-xs font-semibold text-slate-400">Nothing paid yet.</p>
          {:else}
            <ul class="flex flex-col">
              {#each selected.payments as payment (payment.id)}
                <li class="relative flex gap-2.5 border-l-2 border-slate-200 pb-3.5 pl-4 last:border-transparent last:pb-0">
                  <span class="absolute -left-[6px] top-1 h-2.5 w-2.5 rounded-full border-2 border-white bg-teal-600"></span>
                  <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                    <strong class="text-sm text-ink">{money(payment.amount)}</strong>
                    <span class="text-[11px] font-semibold capitalize text-slate-500">{payment.method}{payment.note ? ` · ${payment.note}` : ''}</span>
                  </div>
                  <time class="whitespace-nowrap text-[10px] font-bold text-slate-400">{dateLabel(payment.paid_on)}</time>
                </li>
              {/each}
            </ul>
          {/if}
          {#if selected.balance > 0}
            <div class="grid gap-2.5 sm:grid-cols-3 [&_.field]:mt-0">
              <Field label="Amount">
                <Input inputmode="numeric" bind:value={payAmount} placeholder={String(selected.balance)} />
              </Field>
              <Field label="Method">
                <Select bind:value={payMethod}>
                  {#each paymentMethods as [value, name] (value)}
                    <option {value}>{name}</option>
                  {/each}
                </Select>
              </Field>
              <Field label="Note">
                <Input bind:value={payNote} placeholder="Optional" />
              </Field>
            </div>
            <div class="flex flex-wrap justify-end gap-2">
              <Button variant="secondary" size="sm" onclick={() => (payAmount = String(selected?.balance ?? ''))}>Use full balance</Button>
              <Button size="sm" disabled={saving} onclick={addPayment}>{saving ? 'Saving…' : 'Record payment'}</Button>
            </div>
          {/if}
        </section>
      </div>
    </div>
  </div>
{/if}

{#if creating}
  <div class="sheet-backdrop backdrop-blur-sm max-sm:p-0">
    <button class="job-fade absolute inset-0 cursor-pointer" type="button" aria-label="Close new order" onclick={closeCreate}></button>
    <div class="sheet job-modal relative z-10 flex max-h-[92dvh] w-[min(960px,100%)] flex-col overflow-hidden bg-slate-50 p-0 max-sm:h-dvh max-sm:max-h-dvh max-sm:rounded-none" role="dialog" aria-modal="true" aria-label="New tailor order">
      <form class="flex min-h-0 flex-1 flex-col" onsubmit={(event) => { event.preventDefault(); createOrder() }}>
        <header class="flex flex-none items-start justify-between gap-4 border-b border-slate-200 bg-white px-5 py-5 sm:px-6">
          <div>
            <span class="flex items-center gap-2 text-[11px] font-extrabold uppercase tracking-[0.09em] text-teal-700">
              <span class="h-0.5 w-5 rounded-full bg-current"></span>
              New order
            </span>
            <h2 class="mt-1.5 text-[22px] font-extrabold tracking-tight text-ink">Take a tailor job</h2>
            <p class="mt-1 text-xs font-semibold text-slate-500">Measurements, cloth and the first payment in one pass.</p>
          </div>
          <button
            class="grid h-9 w-9 flex-none place-items-center rounded-xl bg-slate-100 text-slate-600 transition-colors hover:bg-slate-200"
            type="button"
            onclick={closeCreate}
            aria-label="Close"
          >
            <svg class="h-[18px] w-[18px] fill-none stroke-current stroke-[1.9]" viewBox="0 0 24 24" stroke-linecap="round" aria-hidden="true">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </header>

        <div class="flex min-h-0 flex-1 flex-col gap-5 overflow-auto px-5 py-5 sm:px-6">
          <section class="flex flex-col gap-3">
            <div class="flex items-start gap-3">
              <span class="grid h-7 w-7 flex-none place-items-center rounded-lg bg-teal-700 text-xs font-extrabold text-white">1</span>
              <div>
                <h3 class="text-sm font-extrabold tracking-tight text-ink">Customer &amp; garment</h3>
                <p class="mt-0.5 text-[11px] font-semibold text-slate-400">Who the order is for and what is being made.</p>
              </div>
            </div>
            <div class="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3 [&_.field]:mt-0">
              <Field label="Customer name"><Input bind:value={customerName} bind:element={nameInput} placeholder="Full name" /></Field>
              <Field label="Phone"><Input bind:value={phone} placeholder="Optional" /></Field>
              <Field label="Garment"><Input bind:value={garment} placeholder="Shalwar kameez, shirt, waistcoat" /></Field>
            </div>
          </section>

          <section class="flex flex-col gap-3">
            <div class="flex items-start gap-3">
              <span class="grid h-7 w-7 flex-none place-items-center rounded-lg bg-teal-700 text-xs font-extrabold text-white">2</span>
              <div>
                <h3 class="text-sm font-extrabold tracking-tight text-ink">Measurements</h3>
                <p class="mt-0.5 text-[11px] font-semibold text-slate-400">Leave any field blank if it does not apply.</p>
              </div>
            </div>
            <div class="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-5 [&_.field]:mt-0">
              {#each measures as [key, name] (key)}
                <Field label={name}><Input bind:value={measurement[key]} placeholder="—" /></Field>
              {/each}
              <Field label="Other"><Input bind:value={otherMeasure} placeholder="Notes" /></Field>
            </div>
          </section>

          <section class="flex flex-col gap-3">
            <div class="flex items-start gap-3">
              <span class="grid h-7 w-7 flex-none place-items-center rounded-lg bg-teal-700 text-xs font-extrabold text-white">3</span>
              <div>
                <h3 class="text-sm font-extrabold tracking-tight text-ink">Cloth</h3>
                <p class="mt-0.5 text-[11px] font-semibold text-slate-400">What the garment is cut from.</p>
              </div>
            </div>
            <div class="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3 [&_.field]:mt-0">
              <Field label="Type"><Input bind:value={clothType} placeholder="Cotton, linen, wool" /></Field>
              <Field label="Color"><Input bind:value={clothColor} /></Field>
              <Field label="Quantity"><Input bind:value={clothQuantity} placeholder="3.5 m" /></Field>
            </div>
            <div class="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3 [&_.field]:mt-0 [&_.segmented]:mt-0">
              <Field label="Provided by">
                <Segmented label="Cloth provided by" bind:value={clothSource} options={clothSources} />
              </Field>
              <div class="sm:col-span-2 lg:col-span-2">
                <Field label="Cloth notes"><Input bind:value={clothNotes} placeholder="Anything to remember about the cloth" /></Field>
              </div>
            </div>
          </section>

          <section class="flex flex-col gap-3">
            <div class="flex items-start gap-3">
              <span class="grid h-7 w-7 flex-none place-items-center rounded-lg bg-teal-700 text-xs font-extrabold text-white">4</span>
              <div>
                <h3 class="text-sm font-extrabold tracking-tight text-ink">Work, delivery &amp; payment</h3>
                <p class="mt-0.5 text-[11px] font-semibold text-slate-400">Sewing instructions, the promised date and the advance.</p>
              </div>
            </div>
            <Field label="Sewing details"><Textarea bind:value={sewing} rows={3} placeholder="Collar, cuffs, fitting notes…" /></Field>
            <Field label="Other information"><Textarea bind:value={notes} rows={2} placeholder="Anything else about this order" /></Field>
            <div class="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-4 [&_.field]:mt-0">
              <Field label="Delivery date"><Input type="date" bind:value={deliveryDate} /></Field>
              <Field label="Total amount"><Input inputmode="numeric" bind:value={total} placeholder="0" /></Field>
              <Field label="Advance"><Input inputmode="numeric" bind:value={advance} placeholder="Optional" /></Field>
              <Field label="Advance method">
                <Select bind:value={advanceMethod}>
                  {#each paymentMethods as [value, name] (value)}
                    <option {value}>{name}</option>
                  {/each}
                </Select>
              </Field>
            </div>
          </section>
        </div>

        <footer class="flex flex-none flex-wrap items-center justify-between gap-4 border-t border-slate-200 bg-white px-5 py-4 sm:px-6">
          <div class="flex flex-wrap gap-4.5 text-[11px] font-bold text-slate-500">
            {#if preview}
              <span>Total <strong class="ml-1 text-ink">{money(preview.total)}</strong></span>
              {#if preview.advance > 0}
                <span>Advance <strong class="ml-1 text-ink">{money(preview.advance)}</strong></span>
              {/if}
              <span>Balance <strong class="ml-1 {preview.balance > 0 ? 'text-rose-700' : 'text-ink'}">{money(preview.balance)}</strong></span>
            {:else}
              <span class="text-slate-400">The balance shows as you type the total.</span>
            {/if}
          </div>
          <div class="ml-auto flex gap-2 max-sm:w-full">
            <Button class="max-sm:flex-1" variant="secondary" type="button" onclick={closeCreate}>Cancel</Button>
            <Button class="max-sm:flex-1" type="submit" disabled={saving}>{saving ? 'Saving…' : 'Save order'}</Button>
          </div>
        </footer>
      </form>
    </div>
  </div>
{/if}
