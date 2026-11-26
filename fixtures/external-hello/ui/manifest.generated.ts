/* Placeholder until `cargo xtask plugins check` regenerates from Rust. */
import type { UiPlugin } from '../../platform/registry'

export const metadata = {
  id: 'hello',
  requiresBackend: true as const,
  permissions: ['hello.read'] as string[],
  nav: [] as UiPlugin['nav'],
  settingsPages: [] as UiPlugin['settingsPages'],
  reportPages: [] as UiPlugin['reportPages'],
  slotsProvided: [] as string[],
  contributions: [] as Array<{ id: string; slot: string; order: number }>,
}
