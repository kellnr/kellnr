/**
 * Composable for handling API requests with loading and error states
 *
 * Usage:
 * ```ts
 * const { loading, error, data, execute } = useApiRequest(userService.getUsers)
 *
 * // Execute the request
 * await execute()
 *
 * // Or with parameters
 * const { execute: deleteUser } = useApiRequest(userService.deleteUser)
 * await deleteUser('username')
 * ```
 */
import { ref, type Ref } from 'vue'
import type { ApiResult, ApiError } from '../types/api'

export interface UseApiRequestReturn<T, Args extends unknown[]> {
  /** Whether the request is in progress */
  loading: Ref<boolean>
  /** Error from the last request, if any */
  error: Ref<ApiError | null>
  /** Data from the last successful request */
  data: Ref<T | null>
  /** Execute the API request */
  execute: (...args: Args) => Promise<ApiResult<T>>
  /** Reset the state */
  reset: () => void
}

/**
 * Create a reactive wrapper around an API function
 */
export function useApiRequest<T, Args extends unknown[]>(
  apiFn: (...args: Args) => Promise<ApiResult<T>>
): UseApiRequestReturn<T, Args> {
  const loading = ref(false)
  const error = ref<ApiError | null>(null)
  const data = ref<T | null>(null) as Ref<T | null>

  async function execute(...args: Args): Promise<ApiResult<T>> {
    loading.value = true
    error.value = null

    const result = await apiFn(...args)

    loading.value = false

    if (result.error) {
      error.value = result.error
      data.value = null
    } else {
      data.value = result.data
    }

    return result
  }

  function reset() {
    loading.value = false
    error.value = null
    data.value = null
  }

  return {
    loading,
    error,
    data,
    execute,
    reset,
  }
}

/**
 * Simpler hook for one-off API calls without persistent state
 *
 * Usage:
 * ```ts
 * const { loading, execute } = useApiAction()
 *
 * const handleDelete = async (id: number) => {
 *   const result = await execute(() => tokenService.deleteToken(id))
 *   if (result.data !== null) {
 *     // Success
 *   }
 * }
 * ```
 */
export function useApiAction() {
  const loading = ref(false)

  async function execute<T>(apiFn: () => Promise<ApiResult<T>>): Promise<ApiResult<T>> {
    loading.value = true
    const result = await apiFn()
    loading.value = false
    return result
  }

  return {
    loading,
    execute,
  }
}
