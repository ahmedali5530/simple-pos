import { request } from './api'

export async function printReceipt(billNo: string) {
  await request<{ printed: boolean }>('/api/v1/printing/receipt', {
    method: 'POST',
    body: JSON.stringify({ bill_no: billNo }),
  })
}

export function printProductMix(from?: string, to?: string) {
  return request<{ printed: boolean }>('/api/v1/printing/mix', {
    method: 'POST',
    body: JSON.stringify({ from, to }),
  })
}

export function printCashShift(id: number) {
  return request<{ printed: boolean }>('/api/v1/printing/cash', {
    method: 'POST',
    body: JSON.stringify({ id }),
  })
}
