import { downloadWorkbook } from '../../lib/api'
import { request } from '../../platform/http'

export interface PaymentType {
  id: string
  label: string
  enabled: boolean
  requires_reference: boolean
}

export interface PaymentsSettings {
  types: PaymentType[]
}

export interface MethodTotal {
  method: string
  label: string
  amount: number
  bills: number
}

export interface PaymentsReport {
  totals: MethodTotal[]
  collected: number
}

export function loadPaymentsSettings(): Promise<PaymentsSettings> {
  return request('/api/v1/payments/settings')
}

export function savePaymentsSettings(body: PaymentsSettings): Promise<PaymentsSettings> {
  return request('/api/v1/payments/settings', {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function loadPaymentsTypes(): Promise<PaymentsSettings> {
  return request('/api/v1/payments/types')
}

export function loadPaymentsReport(from?: string, to?: string): Promise<PaymentsReport> {
  const query = new URLSearchParams()
  if (from) query.set('from', from)
  if (to) query.set('to', to)
  const suffix = query.size ? `?${query}` : ''
  return request(`/api/v1/payments/report${suffix}`)
}

export async function exportPaymentsReport(format: 'xlsx' | 'pdf', from?: string, to?: string) {
  const params = new URLSearchParams({ format })
  if (from && to) {
    params.set('from', from)
    params.set('to', to)
  }
  await downloadWorkbook(`/api/v1/payments/export?${params}`, `payment-mix.${format}`)
}
