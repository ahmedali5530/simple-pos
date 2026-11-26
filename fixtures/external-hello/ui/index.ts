import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'

export const helloPlugin = defineUiPlugin({
  ...metadata,
  pages: [],
})
