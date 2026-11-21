const settingsPages = ['items', 'sections', 'categories', 'users', 'system', 'plugins'] as const
const reportPages = ['sales', 'ledger', 'mix'] as const

export type SettingsPage = (typeof settingsPages)[number]
export type ReportPage = (typeof reportPages)[number]

function canonical(pathname: string) {
  const path = pathname.length > 1 && pathname.endsWith('/') ? pathname.slice(0, -1) : pathname
  if (path === '/' || path === '') return '/billing'
  if (path === '/settings') return '/settings/items'
  if (path === '/reports') return '/reports/sales'
  return path
}

const here = canonical(window.location.pathname)
if (here !== window.location.pathname) history.replaceState(null, '', here)

export const location = $state({ path: here })

export function navigate(href: string) {
  const next = canonical(href)
  if (next === location.path) return
  history.pushState(null, '', next)
  location.path = next
}

export function followLink(event: MouseEvent, href: string) {
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return
  event.preventDefault()
  navigate(href)
}

window.addEventListener('popstate', () => {
  const next = canonical(window.location.pathname)
  if (next !== window.location.pathname) history.replaceState(null, '', next)
  location.path = next
})

export function screenOf(path: string) {
  if (path === '/billing') return 'billing'
  if (path === '/stock') return 'stock'
  if (path === '/reports' || path.startsWith('/reports/')) return 'reports'
  if (path === '/cash') return 'cash'
  if (path === '/login') return 'login'
  if (path.startsWith('/settings/')) return 'settings'
  return 'missing'
}

export function settingsPage(path: string): SettingsPage {
  const page = path.slice('/settings/'.length)
  return settingsPages.includes(page as SettingsPage) ? (page as SettingsPage) : 'items'
}

export function reportsPage(path: string): ReportPage {
  const page = path.slice('/reports/'.length)
  return reportPages.includes(page as ReportPage) ? (page as ReportPage) : 'sales'
}
