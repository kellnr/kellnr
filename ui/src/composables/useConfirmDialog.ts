/**
 * Composable for reusable confirmation dialogs
 *
 * Usage:
 * ```ts
 * const { isOpen, title, message, confirm, cancel, show } = useConfirmDialog()
 *
 * // In template:
 * // <v-dialog v-model="isOpen">
 * //   <v-card>
 * //     <v-card-title>{{ title }}</v-card-title>
 * //     <v-card-text>{{ message }}</v-card-text>
 * //     <v-card-actions>
 * //       <v-btn @click="cancel">Cancel</v-btn>
 * //       <v-btn @click="confirm">Confirm</v-btn>
 * //     </v-card-actions>
 * //   </v-card>
 * // </v-dialog>
 *
 * // To show dialog:
 * const confirmed = await show({
 *   title: 'Delete User',
 *   message: 'Are you sure you want to delete this user?'
 * })
 *
 * if (confirmed) {
 *   // User confirmed
 * }
 * ```
 */
import { ref, type Ref } from 'vue'

export interface ConfirmDialogOptions {
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  confirmColor?: string
}

export interface UseConfirmDialogReturn {
  /** Whether the dialog is visible */
  isOpen: Ref<boolean>
  /** Dialog title */
  title: Ref<string>
  /** Dialog message */
  message: Ref<string>
  /** Confirm button text */
  confirmText: Ref<string>
  /** Cancel button text */
  cancelText: Ref<string>
  /** Confirm button color */
  confirmColor: Ref<string>
  /** Call this when user confirms */
  confirm: () => void
  /** Call this when user cancels */
  cancel: () => void
  /** Show the dialog and wait for user response */
  show: (options: ConfirmDialogOptions) => Promise<boolean>
}

export function useConfirmDialog(): UseConfirmDialogReturn {
  const isOpen = ref(false)
  const title = ref('')
  const message = ref('')
  const confirmText = ref('Confirm')
  const cancelText = ref('Cancel')
  const confirmColor = ref('primary')

  let resolvePromise: ((value: boolean) => void) | null = null

  function confirm() {
    isOpen.value = false
    if (resolvePromise) {
      resolvePromise(true)
      resolvePromise = null
    }
  }

  function cancel() {
    isOpen.value = false
    if (resolvePromise) {
      resolvePromise(false)
      resolvePromise = null
    }
  }

  function show(options: ConfirmDialogOptions): Promise<boolean> {
    title.value = options.title
    message.value = options.message
    confirmText.value = options.confirmText || 'Confirm'
    cancelText.value = options.cancelText || 'Cancel'
    confirmColor.value = options.confirmColor || 'primary'
    isOpen.value = true

    return new Promise((resolve) => {
      resolvePromise = resolve
    })
  }

  return {
    isOpen,
    title,
    message,
    confirmText,
    cancelText,
    confirmColor,
    confirm,
    cancel,
    show,
  }
}

/**
 * Simpler confirmation dialog that uses a callback pattern
 * (useful for migrating existing code gradually)
 *
 * Usage:
 * ```ts
 * const { dialog, showConfirm } = useConfirmCallback()
 *
 * function handleDelete(name: string) {
 *   showConfirm({
 *     title: 'Delete User',
 *     message: `Delete "${name}"?`,
 *     onConfirm: () => deleteUser(name)
 *   })
 * }
 * ```
 */
export interface ConfirmCallbackOptions extends ConfirmDialogOptions {
  onConfirm: () => void | Promise<void>
}

export function useConfirmCallback() {
  const isOpen = ref(false)
  const title = ref('')
  const message = ref('')
  const confirmText = ref('Confirm')
  const cancelText = ref('Cancel')
  const confirmColor = ref('primary')
  const onConfirmCallback = ref<(() => void | Promise<void>) | null>(null)

  function confirm() {
    isOpen.value = false
    if (onConfirmCallback.value) {
      onConfirmCallback.value()
      onConfirmCallback.value = null
    }
  }

  function cancel() {
    isOpen.value = false
    onConfirmCallback.value = null
  }

  function showConfirm(options: ConfirmCallbackOptions) {
    title.value = options.title
    message.value = options.message
    confirmText.value = options.confirmText || 'Confirm'
    cancelText.value = options.cancelText || 'Cancel'
    confirmColor.value = options.confirmColor || 'primary'
    onConfirmCallback.value = options.onConfirm
    isOpen.value = true
  }

  return {
    dialog: {
      isOpen,
      title,
      message,
      confirmText,
      cancelText,
      confirmColor,
      confirm,
      cancel,
    },
    showConfirm,
  }
}
