import { defineUiPlugin } from '../../platform/registry'
import { metadata } from './manifest.generated'
import './options'

export const matrixPlugin = defineUiPlugin({
  ...metadata,
  pages: [],
  contributions: metadata.contributions.map((entry) => {
    if (entry.id === 'matrix.recipe') {
      return { ...entry, component: () => import('./screens/MatrixFields.svelte') }
    }
    if (entry.id === 'matrix.picker') {
      return { ...entry, component: () => import('./screens/MatrixPicker.svelte') }
    }
    if (entry.id === 'matrix.stock-form') {
      return { ...entry, component: () => import('./screens/MatrixStockForm.svelte') }
    }
    if (entry.id === 'matrix.stock-summary') {
      return { ...entry, component: () => import('./screens/MatrixStockSummary.svelte') }
    }
    return entry
  }),
})
