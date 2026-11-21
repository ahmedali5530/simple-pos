export interface UiNavItem {
  id: string
  label: string
  href: string
  permission?: string
  order: number
}

export interface UiSettingsPage {
  id: string
  label: string
  href: string
  permission?: string
  order: number
}

export interface UiReportPage {
  id: string
  label: string
  href: string
  permission?: string
  order: number
}

export interface UiPlugin {
  id: string
  nav: UiNavItem[]
  settingsPages: UiSettingsPage[]
  reportPages: UiReportPage[]
  permissions: string[]
}

export function defineUiPlugin(plugin: UiPlugin): UiPlugin {
  return plugin
}

export interface PlatformManifest {
  tenant_id: string
  plugins: { id: string; name: string; version: string; nav: UiNavItem[]; settings_pages: UiSettingsPage[]; report_pages: UiReportPage[] }[]
  permissions: string[]
  nav?: UiNavItem[]
  settings_pages?: UiSettingsPage[]
  report_pages?: UiReportPage[]
}

export function filterNav(items: UiNavItem[], permissions: string[]): UiNavItem[] {
  const all = permissions.includes('*')
  return items
    .filter((item) => all || !item.permission || permissions.includes(item.permission))
    .sort((a, b) => a.order - b.order)
}

/** Merge API + static UI nav; API wins on matching ids so frontend-only items (e.g. Plugins) survive. */
export function mergeNav(api: UiNavItem[], local: UiNavItem[]): UiNavItem[] {
  const byId = new Map<string, UiNavItem>()
  for (const item of local) byId.set(item.id, item)
  for (const item of api) byId.set(item.id, item)
  return [...byId.values()].sort((a, b) => a.order - b.order)
}
