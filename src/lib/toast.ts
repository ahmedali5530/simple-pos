import { toast } from 'svelte-sonner'

export function toastError(error: unknown, fallback: string) {
  toast.error(error instanceof Error && error.message ? error.message : fallback)
}

export function toastSaved(message: string) {
  toast.success(message)
}

export { toast }
