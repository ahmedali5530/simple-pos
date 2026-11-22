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

export interface UiPageModule {
  default: import('svelte').Component
}

export interface UiPage {
  path: string
  title: string
  permission?: string
  density?: 'compact' | 'comfortable' | 'touch'
  /** Load the page component. The shell does not import plugin screens itself. */
  load: () => Promise<UiPageModule>
}

export interface UiSlotModule {
  default: import('svelte').Component
}

/** A component another plugin renders inside a named outlet. */
export interface UiSlotContribution {
  id: string
  slot: string
  order: number
  /** Omitted for metadata-only declarations that mirror the Rust manifest. */
  component?: () => Promise<UiSlotModule>
}

export interface UiPlugin {
  id: string
  nav: UiNavItem[]
  settingsPages: UiSettingsPage[]
  reportPages: UiReportPage[]
  pages: UiPage[]
  /**
   * True when the backend compiles this plugin and reports it in the manifest.
   * A frontend-only plugin (the admin screens) sets this to false.
   */
  requiresBackend: boolean
  /** Outlets this plugin's screens host, e.g. `catalog.item-form`. */
  slotsProvided: string[]
  /** Outlets this plugin fills, e.g. `pos.line-source`. */
  contributions: UiSlotContribution[]
  permissions: string[]
}

export function defineUiPlugin(
  plugin: Omit<UiPlugin, 'pages' | 'slotsProvided' | 'contributions' | 'requiresBackend'> &
    Partial<Pick<UiPlugin, 'pages' | 'slotsProvided' | 'contributions' | 'requiresBackend'>>,
): UiPlugin {
  return {
    pages: [],
    slotsProvided: [],
    contributions: [],
    requiresBackend: true,
    ...plugin,
  }
}

export interface PlatformManifest {
  tenant_id: string
  plugins: { id: string; name: string; version: string; nav: UiNavItem[]; settings_pages: UiSettingsPage[]; report_pages: UiReportPage[] }[]
  permissions: string[]
  nav?: UiNavItem[]
  settings_pages?: UiSettingsPage[]
  report_pages?: UiReportPage[]
  available?: { id: string; name: string; version: string; enabled: boolean }[]
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
