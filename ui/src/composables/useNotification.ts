/**
 * Snackbar notifications shared by the whole app. `App.vue` renders the one
 * snackbar; every caller of `useNotification()` changes the same state.
 */
import { reactive } from 'vue'

export interface SnackbarState {
  show: boolean
  message: string
  color: string
  timeout: number
}

export interface UseNotificationReturn {
  /** Reactive snackbar state */
  snackbar: SnackbarState
  /** Show a success notification */
  showSuccess: (message: string, timeout?: number) => void
  /** Show an error notification */
  showError: (message: string, timeout?: number) => void
  /** Show an info notification */
  showInfo: (message: string, timeout?: number) => void
  /** Show a warning notification */
  showWarning: (message: string, timeout?: number) => void
  /** Close the current notification */
  close: () => void
}

const DEFAULT_TIMEOUT = 5000

const snackbar = reactive<SnackbarState>({
  show: false,
  message: '',
  color: 'success',
  timeout: DEFAULT_TIMEOUT,
})

export function useNotification(): UseNotificationReturn {

  function show(message: string, color: string, timeout: number = DEFAULT_TIMEOUT) {
    snackbar.message = message
    snackbar.color = color
    snackbar.timeout = timeout
    snackbar.show = true
  }

  function showSuccess(message: string, timeout?: number) {
    show(message, 'success', timeout)
  }

  function showError(message: string, timeout?: number) {
    show(message, 'error', timeout)
  }

  function showInfo(message: string, timeout?: number) {
    show(message, 'info', timeout)
  }

  function showWarning(message: string, timeout?: number) {
    show(message, 'warning', timeout)
  }

  function close() {
    snackbar.show = false
  }

  return {
    snackbar,
    showSuccess,
    showError,
    showInfo,
    showWarning,
    close,
  }
}
