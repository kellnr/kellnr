/**
 * Composable for centralized snackbar notifications
 *
 * Usage:
 * ```ts
 * const { snackbar, showSuccess, showError, showInfo, close } = useNotification()
 *
 * // In template (once in App.vue or layout):
 * // <v-snackbar
 * //   v-model="snackbar.show"
 * //   :color="snackbar.color"
 * //   :timeout="snackbar.timeout"
 * // >
 * //   {{ snackbar.message }}
 * //   <template v-slot:actions>
 * //     <v-btn @click="close">Close</v-btn>
 * //   </template>
 * // </v-snackbar>
 *
 * // To show notification:
 * showSuccess('User created successfully')
 * showError('Failed to delete user')
 * ```
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

export function useNotification(): UseNotificationReturn {
  const snackbar = reactive<SnackbarState>({
    show: false,
    message: '',
    color: 'success',
    timeout: DEFAULT_TIMEOUT,
  })

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

/**
 * Create a singleton notification instance
 * Use this when you want to share notification state across components
 */
let globalNotification: UseNotificationReturn | null = null

export function useGlobalNotification(): UseNotificationReturn {
  if (!globalNotification) {
    globalNotification = useNotification()
  }
  return globalNotification
}
