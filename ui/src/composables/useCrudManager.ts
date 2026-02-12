import { ref } from 'vue'
import { useStatusMessage, useConfirmCallback, useNotification } from './index'
import { isSuccess } from '../services/api'
import type { ApiResult } from '../types/api'

interface CrudManagerOptions<T> {
  loadFn: () => Promise<ApiResult<T[]>>
  entityName: string
}

export function useCrudManager<T>(options: CrudManagerOptions<T>) {
  const items = ref<T[]>([])
  const status = useStatusMessage()
  const { dialog, showConfirm } = useConfirmCallback()
  const notification = useNotification()

  async function load() {
    const result = await options.loadFn()
    if (isSuccess(result)) {
      items.value = result.data
    }
  }

  async function handleAdd<R>(
    addFn: () => Promise<ApiResult<R>>,
    successMessage: string,
    onSuccess?: () => void
  ) {
    status.clear()
    const result = await addFn()
    if (isSuccess(result)) {
      status.setSuccess(successMessage)
      onSuccess?.()
      await load()
    } else if (result.error) {
      status.setError(result.error.message)
    }
  }

  function handleDelete(
    name: string,
    deleteFn: () => Promise<ApiResult<unknown>>,
    customTitle?: string
  ) {
    showConfirm({
      title: customTitle || `Delete ${options.entityName}`,
      message: `Are you sure you want to delete "${name}"? This action cannot be undone.`,
      confirmColor: 'error',
      onConfirm: async () => {
        const result = await deleteFn()
        if (isSuccess(result)) {
          notification.showSuccess(`${options.entityName} "${name}" deleted`)
          await load()
        } else if (result.error) {
          notification.showError(result.error.message)
        }
      }
    })
  }

  function handleToggle<R>(
    toggleFn: () => Promise<ApiResult<R>>,
    confirmOptions: { title: string; message: string; confirmColor?: string },
    successMessage: string,
    onSuccess?: () => void
  ) {
    showConfirm({
      ...confirmOptions,
      confirmColor: confirmOptions.confirmColor || 'primary',
      onConfirm: async () => {
        const result = await toggleFn()
        if (isSuccess(result)) {
          notification.showSuccess(successMessage)
          onSuccess?.()
        } else if (result.error) {
          notification.showError(result.error.message)
        }
      }
    })
  }

  return {
    items,
    status,
    dialog,
    notification,
    load,
    handleAdd,
    handleDelete,
    handleToggle,
  }
}
