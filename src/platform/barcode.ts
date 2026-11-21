import type { BarcodeRule, ItemVariant, PosItem } from './types'

export interface EmbeddedCode {
  mode: 'weight' | 'price'
  itemCode: string
  value: number
  scale: number
}

export function parseEmbedded(code: string, rules: BarcodeRule[]): EmbeddedCode | null {
  if (!/^\d{13}$/.test(code) || !ean13Ok(code)) return null
  for (const rule of rules) {
    if (!code.startsWith(rule.prefix)) continue
    const itemDigits = 12 - rule.prefix.length - 5
    if (itemDigits < 1) continue
    const itemStart = rule.prefix.length
    const itemCode = code.slice(itemStart, itemStart + itemDigits)
    const value = Number(code.slice(itemStart + itemDigits, 12))
    if (rule.mode !== 'weight' && rule.mode !== 'price') continue
    return { mode: rule.mode, itemCode, value, scale: rule.scale }
  }
  return null
}

export function codesMatch(stored: string, embedded: string): boolean {
  const trim = (value: string) => value.replace(/^0+/, '')
  const left = trim(stored)
  const right = trim(embedded)
  return stored === embedded || (left.length > 0 && left === right)
}

export function embeddedSale(
  unitRate: number,
  code: EmbeddedCode,
): { rate: number; units: number | null; label: string } | null {
  if (code.mode === 'price') {
    const rate = divRound(code.value, code.scale)
    if (rate < 1) return null
    return { rate, units: null, label: '' }
  }
  if (code.value < 1) return null
  const amount = divRound(unitRate * code.value, code.scale)
  if (amount < 1) return null
  return { rate: amount, units: code.value, label: formatScaled(code.value, code.scale) }
}

export function findEmbedded(
  items: PosItem[],
  itemCode: string,
): { item: PosItem; variant: ItemVariant | null } | null {
  for (const item of items) {
    for (const variant of item.variants) {
      if (variant.barcode && codesMatch(variant.barcode, itemCode)) return { item, variant }
    }
  }
  for (const item of items) {
    if (item.barcode && codesMatch(item.barcode, itemCode)) return { item, variant: null }
  }
  return null
}

function ean13Ok(code: string): boolean {
  const digits = [...code].map(Number)
  const sum = digits.slice(0, 12).reduce((total, digit, index) => total + digit * (index % 2 === 0 ? 1 : 3), 0)
  return (10 - (sum % 10)) % 10 === digits[12]
}

export interface PromptQuote {
  rate: number
  units: number
  qty: number
  label: string
}

export function promptSale(
  prompt: 'quantity' | 'price',
  baseRate: number,
  scale: number,
  raw: string,
): PromptQuote | null {
  if (baseRate < 1 || scale < 1) return null
  if (prompt === 'quantity') {
    const milli = parseMilli(raw)
    if (milli == null) return null
    if (milli % 1000 === 0) {
      const qty = milli / 1000
      if (qty < 1) return null
      return { rate: baseRate, units: scale, qty, label: '' }
    }
    if (!Number.isSafeInteger(baseRate * milli) || !Number.isSafeInteger(scale * milli)) return null
    const rate = divRound(baseRate * milli, 1000)
    const units = divRound(scale * milli, 1000)
    if (rate < 1 || units < 1) return null
    return { rate, units, qty: 1, label: formatMilli(milli) }
  }
  if (!/^\d+$/.test(raw.trim())) return null
  const price = Number(raw.trim())
  if (!Number.isSafeInteger(price) || price < 1 || !Number.isSafeInteger(price * scale)) return null
  const units = divRound(price * scale, baseRate)
  if (units < 1) return null
  return { rate: price, units, qty: 1, label: '' }
}

function parseMilli(raw: string): number | null {
  const match = /^(\d+)(?:\.(\d{1,3}))?$/.exec(raw.trim())
  if (!match) return null
  const whole = Number(match[1])
  const frac = (match[2] ?? '').padEnd(3, '0')
  const milli = whole * 1000 + (frac === '' ? 0 : Number(frac))
  if (!Number.isSafeInteger(milli) || milli < 1) return null
  return milli
}

function formatMilli(milli: number): string {
  const whole = Math.floor(milli / 1000)
  const fraction = milli % 1000
  if (fraction === 0) return String(whole)
  return `${whole}.${String(fraction).padStart(3, '0').replace(/0+$/, '')}`
}

function divRound(value: number, scale: number): number {
  return Math.floor((value + Math.floor(scale / 2)) / scale)
}

function formatScaled(value: number, scale: number): string {
  const whole = Math.floor(value / scale)
  const fraction = value % scale
  if (fraction === 0) return String(whole)
  const width = String(scale).length - 1
  const digits = String(fraction).padStart(width, '0').replace(/0+$/, '')
  return `${whole}.${digits}`
}
