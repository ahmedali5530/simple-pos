export interface SessionUser {
  id: number
  name: string
  role: string
  token: string
}

const storageKey = 'till.user'

function read(): SessionUser | null {
  try {
    const raw = localStorage.getItem(storageKey)
    if (!raw) return null
    const user = JSON.parse(raw) as SessionUser
    if (
      !user ||
      typeof user.id !== 'number' ||
      typeof user.name !== 'string' ||
      typeof user.role !== 'string' ||
      typeof user.token !== 'string' ||
      !user.token
    ) {
      return null
    }
    return user
  } catch {
    return null
  }
}

export const session = $state<{ user: SessionUser | null }>({ user: read() })

export function signIn(user: SessionUser) {
  session.user = user
  localStorage.setItem(storageKey, JSON.stringify(user))
}

export function signOut() {
  session.user = null
  localStorage.removeItem(storageKey)
}
