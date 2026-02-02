<template>
  <div>
    <SectionHeader icon="mdi-hammer-wrench" title="Toolchain Management" :count="toolchains.length" />

    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Manage Rust toolchains for distribution via rustup. Upload toolchain archives (.tar.xz) and assign them to release channels.
      </p>

      <!-- Toolchains List -->
      <div v-if="toolchains.length > 0" class="toolchains-section mb-6">
        <SubsectionHeader icon="mdi-package-variant" title="Available Toolchains" />

        <div class="toolchain-list">
          <v-expansion-panels variant="accordion" class="toolchain-panels">
            <v-expansion-panel v-for="toolchain in toolchains" :key="`${toolchain.name}-${toolchain.version}`">
              <v-expansion-panel-title class="panel-title">
                <div class="toolchain-info">
                  <div class="toolchain-avatar">
                    <v-icon icon="mdi-package-variant-closed" size="small"></v-icon>
                  </div>
                  <div class="toolchain-details">
                    <span class="toolchain-name">{{ toolchain.name }} {{ toolchain.version }}</span>
                    <span class="toolchain-date">{{ toolchain.date }} · {{ toolchain.targets.length }} target(s)</span>
                  </div>
                  <v-chip
                    v-if="toolchain.channel"
                    size="x-small"
                    :color="getChannelColor(toolchain.channel)"
                    variant="tonal"
                    class="ms-2"
                  >
                    {{ toolchain.channel }}
                  </v-chip>
                </div>
              </v-expansion-panel-title>
              <v-expansion-panel-text>
                <!-- Channel Assignment -->
                <div class="channel-assignment mb-4">
                  <div class="channel-row">
                    <div class="channel-label">
                      <v-icon icon="mdi-source-branch" size="x-small" class="me-2 text-medium-emphasis"></v-icon>
                      <span class="text-body-2">Release Channel</span>
                    </div>
                    <div class="channel-controls">
                      <v-select
                        v-model="toolchain.channel"
                        :items="channelOptions"
                        density="compact"
                        variant="outlined"
                        hide-details
                        clearable
                        placeholder="None"
                        class="channel-select"
                        @update:model-value="handleChannelChange(toolchain)"
                      ></v-select>
                    </div>
                  </div>
                  <p class="text-caption text-medium-emphasis mt-1 mb-0">
                    Assign this toolchain to a channel so users can install it via <code>rustup install {{ toolchain.channel || 'channel-name' }}</code>
                  </p>
                </div>

                <!-- Targets List -->
                <div class="targets-section">
                  <div class="targets-header">
                    <span class="text-body-2 font-weight-medium">Targets</span>
                    <v-btn
                      color="error"
                      variant="text"
                      size="small"
                      @click.stop="handleDeleteToolchain(toolchain.name, toolchain.version)"
                    >
                      <v-icon icon="mdi-delete-outline" size="small" class="me-1"></v-icon>
                      Delete All
                    </v-btn>
                  </div>
                  <div class="targets-list">
                    <ListItem
                      v-for="target in toolchain.targets"
                      :key="target.target"
                      icon="mdi-chip"
                      compact
                      test-id="target-item"
                    >
                      <template #title>
                        <span class="target-name">{{ target.target }}</span>
                        <span class="target-size">{{ formatSize(target.size) }}</span>
                      </template>
                      <template #actions>
                        <v-btn
                          color="error"
                          variant="text"
                          size="small"
                          @click.stop="handleDeleteTarget(toolchain.name, toolchain.version, target.target)"
                        >
                          <v-icon icon="mdi-delete-outline" size="small"></v-icon>
                        </v-btn>
                      </template>
                    </ListItem>
                  </div>
                </div>
              </v-expansion-panel-text>
            </v-expansion-panel>
          </v-expansion-panels>
        </div>
      </div>

      <EmptyState
        v-else
        icon="mdi-hammer-wrench"
        message="No toolchains uploaded yet."
        class="mb-6"
      />

      <!-- Upload Section -->
      <FormSection icon="mdi-upload" title="Upload Toolchain">
        <v-form @submit.prevent="handleUpload" class="upload-form">
          <div class="form-grid">
            <v-text-field
              v-model="uploadForm.name"
              label="Component Name"
              placeholder="rust"
              prepend-inner-icon="mdi-package-variant"
              variant="outlined"
              density="comfortable"
              hide-details
            ></v-text-field>

            <v-text-field
              v-model="uploadForm.version"
              label="Version"
              placeholder="1.75.0"
              prepend-inner-icon="mdi-tag"
              variant="outlined"
              density="comfortable"
              hide-details
            ></v-text-field>

            <v-text-field
              v-model="uploadForm.target"
              label="Target Triple"
              placeholder="x86_64-unknown-linux-gnu"
              prepend-inner-icon="mdi-chip"
              variant="outlined"
              density="comfortable"
              hide-details
            ></v-text-field>

            <v-text-field
              v-model="uploadForm.date"
              label="Release Date"
              placeholder="2024-01-15"
              prepend-inner-icon="mdi-calendar"
              variant="outlined"
              density="comfortable"
              hide-details
            ></v-text-field>

            <v-select
              v-model="uploadForm.channel"
              :items="channelOptions"
              label="Channel (Optional)"
              prepend-inner-icon="mdi-source-branch"
              variant="outlined"
              density="comfortable"
              hide-details
              clearable
            ></v-select>
          </div>

          <div class="file-upload-area mt-4">
            <input
              ref="fileInput"
              type="file"
              accept=".tar.xz,.tar.gz,.xz"
              @change="handleFileSelect"
              class="d-none"
            />
            <div
              class="drop-zone"
              :class="{ 'drag-over': isDragging, 'has-file': uploadForm.file }"
              @dragover.prevent="isDragging = true"
              @dragleave.prevent="isDragging = false"
              @drop.prevent="handleDrop"
              @click="triggerFileInput"
            >
              <v-icon
                :icon="uploadForm.file ? 'mdi-file-check' : 'mdi-cloud-upload'"
                size="large"
                :color="uploadForm.file ? 'success' : 'primary'"
                class="mb-2"
              ></v-icon>
              <p v-if="uploadForm.file" class="text-body-2 mb-0">
                {{ uploadForm.file.name }} ({{ formatSize(uploadForm.file.size) }})
              </p>
              <p v-else class="text-body-2 text-medium-emphasis mb-0">
                Drop .tar.xz file here or click to browse
              </p>
            </div>
          </div>

          <div class="form-actions mt-4">
            <v-btn
              color="primary"
              type="submit"
              size="large"
              :loading="isUploading"
              :disabled="!canUpload"
            >
              <v-icon icon="mdi-upload" size="small" class="me-2"></v-icon>
              Upload Toolchain
            </v-btn>
          </div>
        </v-form>

        <v-alert
          v-if="uploadStatus.hasStatus"
          :type="uploadStatus.isSuccess ? 'success' : 'error'"
          variant="tonal"
          class="mt-4"
          closable
          @click:close="uploadStatus.clear()"
        >
          {{ uploadStatus.message }}
        </v-alert>
      </FormSection>
    </div>

    <!-- Snackbar for notifications -->
    <NotificationSnackbar
      :snackbar="notification.snackbar"
      @close="notification.close()"
    />

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      v-model="dialogOpen"
      :title="dialog.title"
      :message="dialog.message"
      :confirm-color="dialog.confirmColor"
      @confirm="dialog.confirm()"
      @cancel="dialog.cancel()"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import { useStatusMessage, useConfirmCallback, useNotification } from "../composables"
import { toolchainService } from "../services"
import { isSuccess } from "../services/api"
import type { Toolchain } from "../types/toolchain"
import {
  SectionHeader,
  SubsectionHeader,
  ListItem,
  EmptyState,
  FormSection,
  ConfirmDialog,
  NotificationSnackbar,
} from "./common"

// State
const toolchains = ref<Toolchain[]>([])
const isDragging = ref(false)
const isUploading = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const uploadForm = ref({
  name: "rust",
  version: "",
  target: "",
  date: "",
  channel: null as string | null,
  file: null as File | null,
})

// Composables
const uploadStatus = useStatusMessage()
const { dialog, showConfirm } = useConfirmCallback()
const notification = useNotification()

// Computed wrapper for dialog.isOpen
const dialogOpen = computed({
  get: () => dialog.isOpen.value,
  set: (val: boolean) => { dialog.isOpen.value = val }
})

const channelOptions = ["stable", "beta", "nightly"]

const canUpload = computed(() => {
  return (
    uploadForm.value.name.trim() &&
    uploadForm.value.version.trim() &&
    uploadForm.value.target.trim() &&
    uploadForm.value.date.trim() &&
    uploadForm.value.file
  )
})

// Lifecycle
onBeforeMount(() => {
  loadToolchains()
})

// Load toolchains from API
async function loadToolchains() {
  const result = await toolchainService.getToolchains()
  if (isSuccess(result)) {
    toolchains.value = result.data
  }
}

// File handling
function triggerFileInput() {
  fileInput.value?.click()
}

function handleFileSelect(event: Event) {
  const target = event.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    uploadForm.value.file = target.files[0]
  }
}

function handleDrop(event: DragEvent) {
  isDragging.value = false
  if (event.dataTransfer?.files && event.dataTransfer.files.length > 0) {
    uploadForm.value.file = event.dataTransfer.files[0]
  }
}

// Upload toolchain
async function handleUpload() {
  if (!canUpload.value || !uploadForm.value.file) return

  isUploading.value = true
  uploadStatus.clear()

  const result = await toolchainService.uploadToolchain(
    {
      name: uploadForm.value.name.trim(),
      version: uploadForm.value.version.trim(),
      target: uploadForm.value.target.trim(),
      date: uploadForm.value.date.trim(),
      channel: uploadForm.value.channel || undefined,
    },
    uploadForm.value.file
  )

  isUploading.value = false

  if (isSuccess(result)) {
    uploadStatus.setSuccess("Toolchain uploaded successfully.")
    // Reset form
    uploadForm.value.version = ""
    uploadForm.value.target = ""
    uploadForm.value.date = ""
    uploadForm.value.channel = null
    uploadForm.value.file = null
    if (fileInput.value) {
      fileInput.value.value = ""
    }
    await loadToolchains()
  } else {
    uploadStatus.setError(result.error.message)
  }
}

// Channel change handler
async function handleChannelChange(toolchain: Toolchain) {
  if (toolchain.channel) {
    const result = await toolchainService.setChannel(
      toolchain.channel,
      toolchain.name,
      toolchain.version
    )
    if (isSuccess(result)) {
      notification.showSuccess(`Channel "${toolchain.channel}" assigned`)
      await loadToolchains()
    } else {
      notification.showError(result.error.message)
      await loadToolchains()
    }
  }
}

// Delete entire toolchain
function handleDeleteToolchain(name: string, version: string) {
  showConfirm({
    title: "Delete Toolchain",
    message: `Are you sure you want to delete "${name} ${version}" and ALL its targets? This action cannot be undone.`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await toolchainService.deleteToolchain(name, version)
      if (isSuccess(result)) {
        notification.showSuccess(`Toolchain "${name} ${version}" deleted`)
        await loadToolchains()
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

// Delete target
function handleDeleteTarget(name: string, version: string, target: string) {
  showConfirm({
    title: "Delete Target",
    message: `Are you sure you want to delete target "${target}" from ${name} ${version}? This action cannot be undone.`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await toolchainService.deleteToolchainTarget(name, version, target)
      if (isSuccess(result)) {
        notification.showSuccess(`Target "${target}" deleted`)
        await loadToolchains()
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

// Helpers
function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i]
}

function getChannelColor(channel: string): string {
  switch (channel) {
    case "stable": return "success"
    case "beta": return "warning"
    case "nightly": return "info"
    default: return "primary"
  }
}
</script>

<style scoped>
.section-content {
  padding: 24px;
}

/* Toolchain Panels */
.toolchain-panels {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.toolchain-panels :deep(.v-expansion-panel) {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px !important;
  margin-bottom: 0;
}

.toolchain-panels :deep(.v-expansion-panel::before) {
  box-shadow: none;
}

.toolchain-panels :deep(.v-expansion-panel--active) {
  border-color: rgb(var(--v-theme-primary));
}

.panel-title {
  min-height: 56px;
}

.toolchain-info {
  display: flex;
  align-items: center;
  width: 100%;
}

.toolchain-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.15);
  color: rgb(var(--v-theme-primary));
  margin-right: 12px;
}

.toolchain-details {
  display: flex;
  flex-direction: column;
}

.toolchain-name {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.toolchain-date {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
}

/* Channel Assignment */
.channel-assignment {
  padding: 12px 16px;
  background: rgba(var(--v-theme-primary), 0.03);
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
}

.channel-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.channel-label {
  display: flex;
  align-items: center;
}

.channel-controls {
  flex: 0 0 180px;
}

.channel-select {
  font-size: 14px;
}

.channel-select :deep(.v-field) {
  border-radius: 6px;
}

/* Targets Section */
.targets-section {
  margin-top: 8px;
}

.targets-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.targets-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.target-name {
  font-family: 'Roboto Mono', monospace;
  font-size: 13px;
  color: rgb(var(--v-theme-on-surface));
}

.target-size {
  font-size: 12px;
  color: rgb(var(--v-theme-on-surface-variant));
  background: rgba(var(--v-theme-primary), 0.08);
  padding: 2px 8px;
  border-radius: 4px;
  margin-left: 8px;
}

/* Upload Form */
.upload-form {
  margin-top: 4px;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
}

.form-grid :deep(.v-field) {
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
}

.file-upload-area {
  width: 100%;
}

.drop-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 32px;
  border: 2px dashed rgb(var(--v-theme-outline));
  border-radius: 8px;
  background: rgb(var(--v-theme-surface));
  cursor: pointer;
  transition: all 0.2s ease;
}

.drop-zone:hover,
.drop-zone.drag-over {
  border-color: rgb(var(--v-theme-primary));
  background: rgba(var(--v-theme-primary), 0.05);
}

.drop-zone.has-file {
  border-color: rgb(var(--v-theme-success));
  background: rgba(var(--v-theme-success), 0.05);
}

.form-actions {
  display: flex;
  justify-content: flex-end;
}

/* Responsive */
@media (max-width: 600px) {
  .section-content {
    padding: 20px;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }

  .channel-row {
    flex-direction: column;
    align-items: stretch;
  }

  .channel-controls {
    flex: 1;
  }
}
</style>
