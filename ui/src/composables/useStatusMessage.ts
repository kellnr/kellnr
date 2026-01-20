/**
 * Composable for managing status messages (success/error feedback)
 *
 * Usage:
 * ```ts
 * const status = useStatusMessage()
 *
 * // In template: v-if="status.hasStatus" :type="status.isSuccess ? 'success' : 'error'"
 * // After API call
 * if (result.error) {
 *   status.setError(result.error.message)
 * } else {
 *   status.setSuccess('Operation completed successfully')
 * }
 * ```
 */
import { reactive } from 'vue'

export type StatusType = 'success' | 'error' | ''

export interface StatusMessage {
  status: StatusType
  message: string
}

export function useStatusMessage(autoClearDelay?: number) {
  let timeoutId: ReturnType<typeof setTimeout> | null = null

  const state = reactive({
    status: '' as StatusType,
    message: '',
    // Computed-like getters work in reactive objects
    get isSuccess() {
      return this.status === 'success'
    },
    get isError() {
      return this.status === 'error'
    },
    get hasStatus() {
      return this.status !== ''
    },
    setSuccess(msg: string) {
      this.status = 'success'
      this.message = msg
      scheduleAutoClear()
    },
    setError(msg: string) {
      this.status = 'error'
      this.message = msg
      scheduleAutoClear()
    },
    clear() {
      this.status = ''
      this.message = ''
      if (timeoutId) {
        clearTimeout(timeoutId)
        timeoutId = null
      }
    },
    setFromResult<T>(
      result: { data: T | null; error: { message: string } | null },
      successMessage: string
    ) {
      if (result.error) {
        this.setError(result.error.message)
      } else {
        this.setSuccess(successMessage)
      }
    },
  })

  function scheduleAutoClear() {
    if (autoClearDelay && autoClearDelay > 0) {
      if (timeoutId) {
        clearTimeout(timeoutId)
      }
      timeoutId = setTimeout(() => state.clear(), autoClearDelay)
    }
  }

  return state
}
