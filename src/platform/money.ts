export function formatCost(cents: number, currency: string, locale: string): string {
  const major = cents / 100
  const whole = Number.isInteger(major)
  try {
    return new Intl.NumberFormat(locale, {
      style: 'currency',
      currency,
      minimumFractionDigits: whole ? 0 : 2,
      maximumFractionDigits: 2,
    }).format(major)
  } catch {
    return `${major} ${currency}`
  }
}

export function moneyToCents(text: string): number | null {
  const trimmed = text.trim()
  if (!/^\d+(\.\d{1,2})?$/.test(trimmed)) return null
  const [major, frac = ''] = trimmed.split('.')
  return Number(major) * 100 + Number((frac + '00').slice(0, 2))
}

export function centsToInput(cents: number): string {
  const sign = cents < 0 ? '-' : ''
  const abs = Math.abs(cents)
  const major = Math.trunc(abs / 100)
  const frac = abs % 100
  if (frac === 0) return `${sign}${major}`
  return `${sign}${major}.${String(frac).padStart(2, '0')}`
}

export function nextAverageCost(onHand: number, averageCents: number, qty: number, totalMajor: number): number {
  const unit = Math.floor((totalMajor * 100 + Math.floor(qty / 2)) / qty)
  if (onHand <= 0 || averageCents <= 0) return unit
  return Math.floor((onHand * averageCents + qty * unit + Math.floor((onHand + qty) / 2)) / (onHand + qty))
}

export function formatMoney(amount: number, currency: string, locale: string): string {
  try {
    return new Intl.NumberFormat(locale, {
      style: 'currency',
      currency,
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(amount)
  } catch {
    return `${amount} ${currency}`
  }
}
