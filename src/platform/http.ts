import { session, signOut } from './session.svelte'

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers)
  if (!(init?.body instanceof FormData) && !headers.has('content-type') && init?.body) {
    headers.set('content-type', 'application/json')
  }
  if (session.user?.token && !headers.has('authorization')) {
    headers.set('authorization', `Bearer ${session.user.token}`)
  }
  const response = await fetch(path, {
    ...init,
    headers,
  })
  if (response.status === 204) return undefined as T
  const body = (await response.json().catch(() => ({}))) as T & { error?: string }
  if (response.status === 401) signOut()
  if (!response.ok) {
    throw new Error(body.error || `Request failed (${response.status})`)
  }
  return body
}
