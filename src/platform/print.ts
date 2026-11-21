import { request } from './api'

export async function printReceipt(billNo: string) {
  await request<{ printed: boolean }>('/api/print', {
    method: 'POST',
    body: JSON.stringify({ bill_no: billNo }),
  })
}

export function printProductMix(from?: string, to?: string) {
  return request<{ printed: boolean }>('/api/print/mix', {
    method: 'POST',
    body: JSON.stringify({ from, to }),
  })
}

export function printCashShift(id: number) {
  return request<{ printed: boolean }>('/api/print/cash', {
    method: 'POST',
    body: JSON.stringify({ id }),
  })
}
