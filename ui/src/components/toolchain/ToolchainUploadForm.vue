<template>
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
</template>

<script setup lang="ts">
import { ref, computed } from "vue"
import { useStatusMessage } from "../../composables"
import { toolchainService } from "../../services"
import { isSuccess } from "../../services/api"
import { FormSection } from "../common"

defineProps<{
  channelOptions: string[]
}>()

const emit = defineEmits<{
  (e: 'upload-success'): void
}>()

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

const uploadStatus = useStatusMessage()

const canUpload = computed(() => {
  return (
    uploadForm.value.name.trim() &&
    uploadForm.value.version.trim() &&
    uploadForm.value.target.trim() &&
    uploadForm.value.date.trim() &&
    uploadForm.value.file
  )
})

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
    uploadForm.value.version = ""
    uploadForm.value.target = ""
    uploadForm.value.date = ""
    uploadForm.value.channel = null
    uploadForm.value.file = null
    if (fileInput.value) {
      fileInput.value.value = ""
    }
    emit('upload-success')
  } else {
    uploadStatus.setError(result.error.message)
  }
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i]
}
</script>

<style scoped>
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
  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
