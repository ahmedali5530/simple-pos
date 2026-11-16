import type { PosSnapshot, PostedBill } from './types'

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...(init?.headers ?? {}),
    },
  })
  const body = (await response.json().catch(() => ({}))) as T & { error?: string }
  if (!response.ok) {
    throw new Error(body.error || `Request failed (${response.status})`)
  }
  return body
}

export function loadPos(): Promise<PosSnapshot> {
  return request('/api/pos')
}

export function rememberSection(sectionId: number): Promise<{ last_section_id: number }> {
  return request('/api/pos/focus', {
    method: 'PUT',
    body: JSON.stringify({ section_id: sectionId }),
  })
}

export function saveSectionLayout(
  sections: { id: number; show_on_pos: boolean }[],
): Promise<PosSnapshot> {
  return request('/api/pos/sections', {
    method: 'PUT',
    body: JSON.stringify({ sections }),
  })
}

export function saveItemPrompt(
  itemId: number,
  body: { prompt: 'none' | 'quantity' | 'price'; base_rate: number | null; scale: number },
): Promise<PosSnapshot> {
  return request(`/api/items/${itemId}/prompt`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function postBill(
  sectionId: number,
  lines: {
    item_id: number
    variant_id: number | null
    rate: number
    qty: number
    code: string | null
    entry: string | null
  }[],
): Promise<PostedBill> {
  return request('/api/bills', {
    method: 'POST',
    body: JSON.stringify({ section_id: sectionId, lines }),
  })
}
