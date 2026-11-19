export type ReportPreset = 'today' | 'yesterday' | 'thisWeek' | 'lastWeek' | 'thisMonth' | 'all' | 'custom'

export type ReportSpan = { kind: 'ready'; from?: string; to?: string } | { kind: 'incomplete' } | { kind: 'invalid' }

function day(date: Date, offset = 0) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + offset)
}

function monday(today: Date) {
  const dayIndex = today.getDay()
  const offset = dayIndex === 0 ? -6 : 1 - dayIndex
  return day(today, offset)
}

function parseDate(value: string) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!match) return null
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]))
  if (date.getFullYear() !== Number(match[1]) || date.getMonth() !== Number(match[2]) - 1) return null
  return date
}

export function reportSpan(preset: ReportPreset, fromDate: string, toDate: string, now = new Date()): ReportSpan {
  const today = day(now)
  if (preset === 'all') return { kind: 'ready' }
  if (preset === 'today') return { kind: 'ready', from: today.toISOString(), to: day(today, 1).toISOString() }
  if (preset === 'yesterday') return { kind: 'ready', from: day(today, -1).toISOString(), to: today.toISOString() }
  if (preset === 'thisWeek') return { kind: 'ready', from: monday(today).toISOString(), to: day(today, 1).toISOString() }
  if (preset === 'lastWeek') {
    const start = monday(today)
    return { kind: 'ready', from: day(start, -7).toISOString(), to: start.toISOString() }
  }
  if (preset === 'thisMonth') {
    const first = new Date(today.getFullYear(), today.getMonth(), 1)
    const next = new Date(today.getFullYear(), today.getMonth() + 1, 1)
    return { kind: 'ready', from: first.toISOString(), to: next.toISOString() }
  }
  if (!fromDate || !toDate) return { kind: 'incomplete' }
  const from = parseDate(fromDate)
  const end = parseDate(toDate)
  if (!from || !end) return { kind: 'invalid' }
  const to = day(end, 1)
  if (to.getTime() <= from.getTime()) return { kind: 'invalid' }
  return { kind: 'ready', from: from.toISOString(), to: to.toISOString() }
}
