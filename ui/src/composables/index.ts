/**
 * Composables barrel export
 *
 * Usage:
 * ```ts
 * import { useStatusMessage, useApiRequest, useConfirmDialog } from '@/composables'
 * ```
 */

export { useStatusMessage } from './useStatusMessage'
export type { StatusType, StatusMessage } from './useStatusMessage'

export { useApiRequest, useApiAction } from './useApiRequest'
export type { UseApiRequestReturn } from './useApiRequest'

export { useConfirmDialog, useConfirmCallback } from './useConfirmDialog'
export type {
  ConfirmDialogOptions,
  UseConfirmDialogReturn,
  ConfirmCallbackOptions,
} from './useConfirmDialog'

export { useNotification, useGlobalNotification } from './useNotification'
export type { SnackbarState, UseNotificationReturn } from './useNotification'

export { useCrudManager } from './useCrudManager'
