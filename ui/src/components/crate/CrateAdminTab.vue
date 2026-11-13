<template>
  <v-card class="mb-4 content-card admin-card" elevation="0">
    <div class="admin-header">
      <v-icon icon="mdi-shield-alert" size="small" class="admin-icon"></v-icon>
      <span class="admin-title">Danger zone</span>
    </div>
    <v-card-text class="admin-content">
      <v-alert type="warning" variant="tonal" class="mb-5 admin-alert">
        <div class="admin-alert-content">
          <span>
            Consider yanking a version instead of deleting it. Yanking prevents new dependencies
            but does not break existing ones.
          </span>
        </div>
        <v-btn variant="outlined" class="mt-3" height="36" prepend-icon="mdi-tag-arrow-down-outline"
          data-testid="admin-show-versions" @click="emit('show-versions')">
          Yank in the Versions tab
        </v-btn>
      </v-alert>

      <div class="admin-actions">
        <!-- Delete version -->
        <div class="admin-action-card destructive">
          <div class="admin-action-info">
            <div class="admin-action-header">
              <v-icon icon="mdi-tag-remove" size="small" class="admin-action-icon"></v-icon>
              <span class="admin-action-title">Delete version</span>
            </div>
            <p class="admin-action-desc">
              Permanently delete the selected version of this crate. This action cannot be undone.
            </p>
            <v-select v-model="targetVersion" :items="versionItems" label="Version" variant="outlined"
              density="comfortable" hide-details class="admin-version-select mt-3"
              data-testid="admin-version-select" />
          </div>
          <v-btn color="error" variant="flat" height="40" :disabled="!targetVersion"
            @click="openDeleteVersion" data-testid="admin-delete-version">
            <v-icon icon="mdi-delete-outline" size="small" class="me-2"></v-icon>
            Delete version
          </v-btn>
        </div>

        <!-- Delete entire crate -->
        <div class="admin-action-card destructive">
          <div class="admin-action-info">
            <div class="admin-action-header">
              <v-icon icon="mdi-delete-forever" size="small" class="admin-action-icon"></v-icon>
              <span class="admin-action-title">Delete entire crate</span>
            </div>
            <p class="admin-action-desc">
              Permanently delete <strong>all versions</strong> of <code>{{ crateName }}</code>.
              This will break all crates that depend on it.
            </p>
          </div>
          <v-btn color="error" variant="flat" height="40" @click="openDeleteCrate" data-testid="admin-delete-crate">
            <v-icon icon="mdi-delete-forever" size="small" class="me-2"></v-icon>
            Delete crate
          </v-btn>
        </div>
      </div>

      <v-alert v-if="errorMessage" type="error" variant="tonal" closable class="mt-4"
        data-testid="admin-error" @click:close="errorMessage = ''">
        {{ errorMessage }}
      </v-alert>
    </v-card-text>

    <ConfirmDialog v-model="dialogOpen" :title="dialog.title" :message="dialog.message"
      sub-message="This action cannot be undone." :confirm-text="dialog.confirmText" confirm-color="error"
      confirm-icon="mdi-delete" :required-input="dialog.requiredInput" @confirm="confirmDelete" />
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ConfirmDialog } from '../common'
import { crateService } from '../../services'
import { isSuccess } from '../../services/api'
import { versionSelectItems } from '../../common/crate'
import type { CrateVersionData } from '../../types/crate_data'

const props = defineProps<{
  crateName: string
  version: string
  versions: CrateVersionData[]
}>()

const emit = defineEmits<{
  'show-versions': []
}>()

const router = useRouter()

const versionItems = computed(() => versionSelectItems(props.versions))
const targetVersion = ref(props.version)
watch(() => props.version, (version) => { targetVersion.value = version })

type DeleteTarget = 'version' | 'crate'

const dialogOpen = ref(false)
const deleteTarget = ref<DeleteTarget>('version')
const errorMessage = ref('')

const dialog = computed(() => deleteTarget.value === 'crate'
  ? {
    title: 'Delete crate',
    message: `Delete all versions of "${props.crateName}"?`,
    confirmText: 'Delete crate',
    requiredInput: props.crateName,
  }
  : {
    title: 'Delete version',
    message: `Delete "${props.crateName}" version "${targetVersion.value}"?`,
    confirmText: 'Delete version',
    requiredInput: undefined,
  })

function openDeleteVersion() {
  deleteTarget.value = 'version'
  dialogOpen.value = true
}

function openDeleteCrate() {
  deleteTarget.value = 'crate'
  dialogOpen.value = true
}

async function confirmDelete() {
  errorMessage.value = ''
  const result = deleteTarget.value === 'crate'
    ? await crateService.deleteCrate(props.crateName)
    : await crateService.deleteVersion(props.crateName, targetVersion.value)
  if (isSuccess(result)) {
    router.push({ name: 'Crates' })
  } else {
    errorMessage.value = `Failed to delete ${deleteTarget.value === 'crate' ? 'the crate' : `version ${targetVersion.value}`}: ${result.error.message}`
  }
}
</script>

<style scoped>
/* Content Cards */
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius-card);
}

/* Admin Card Styles */
.admin-card {
  overflow: hidden;
  border-color: rgba(var(--v-theme-error), 0.3) !important;
}

.admin-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  background: rgba(var(--v-theme-error), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-error), 0.2);
}

.admin-icon {
  color: rgb(var(--v-theme-error));
  margin-right: 12px;
}

.admin-title {
  font-weight: 600;
  font-size: 16px;
  color: rgb(var(--v-theme-error));
}

.admin-content {
  padding: 20px !important;
}

.admin-alert {
  border-radius: var(--kellnr-radius);
}

.admin-alert-content {
  display: flex;
  align-items: flex-start;
}

.admin-alert-content .v-icon {
  flex-shrink: 0;
  margin-top: 2px;
}

.admin-version-select {
  max-width: 260px;
}

.admin-actions {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.admin-action-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 20px;
  background: rgb(var(--v-theme-surface-variant));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius);
  transition: all 0.2s ease;
}

.admin-action-card:hover {
  border-color: rgba(var(--v-theme-error), 0.5);
}

.admin-action-card.destructive {
  background: rgba(var(--v-theme-error), 0.05);
  border-color: rgba(var(--v-theme-error), 0.3);
}

.admin-action-card.destructive:hover {
  border-color: rgb(var(--v-theme-error));
  background: rgba(var(--v-theme-error), 0.08);
}

.admin-action-info {
  flex: 1;
}

.admin-action-header {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
}

.admin-action-icon {
  color: rgb(var(--v-theme-error));
  margin-right: 10px;
}

.admin-action-title {
  font-weight: 600;
  font-size: 15px;
  color: rgb(var(--v-theme-on-surface));
}

.admin-action-desc {
  font-size: 14px;
  line-height: 1.5;
  color: rgb(var(--v-theme-on-surface-variant));
  margin: 0;
}

.admin-action-desc code {
  background: rgb(var(--v-theme-surface));
  padding: 2px 6px;
  border-radius: var(--kellnr-radius);
  font-size: 13px;
  font-family: var(--kellnr-font-mono);
  color: rgb(var(--v-theme-primary));
}

/* Responsive Admin */
@media (max-width: 768px) {
  .admin-action-card {
    flex-direction: column;
    align-items: flex-start;
  }

  .admin-action-card .v-btn {
    width: 100%;
  }
}
</style>
