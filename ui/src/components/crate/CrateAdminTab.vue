<template>
  <v-card class="mb-4 content-card admin-card" elevation="0">
    <div class="admin-header">
      <v-icon icon="mdi-shield-alert" size="small" class="admin-icon"></v-icon>
      <span class="admin-title">Danger Zone</span>
    </div>
    <v-card-text class="admin-content">
      <v-alert type="warning" variant="tonal" class="mb-5 admin-alert">
        <div class="admin-alert-content">
          <v-icon icon="mdi-lightbulb-outline" size="small" class="me-2"></v-icon>
          <span>
            Consider <a href="https://doc.rust-lang.org/cargo/commands/cargo-yank.html" target="_blank" class="admin-link">yanking</a>
            the crate instead of deleting it. Yanking prevents new dependencies but doesn't break existing ones.
          </span>
        </div>
      </v-alert>

      <div class="admin-actions">
        <!-- Delete Version -->
        <div class="admin-action-card destructive">
          <div class="admin-action-info">
            <div class="admin-action-header">
              <v-icon icon="mdi-tag-remove" size="small" class="admin-action-icon"></v-icon>
              <span class="admin-action-title">Delete Version</span>
            </div>
            <p class="admin-action-desc">
              Permanently delete version <code>{{ version }}</code> of this crate.
              This action cannot be undone.
            </p>
          </div>
          <v-btn color="error" variant="flat" @click="handleDeleteVersion">
            <v-icon icon="mdi-delete-outline" size="small" class="me-2"></v-icon>
            Delete Version
          </v-btn>
        </div>

        <!-- Delete Entire Crate -->
        <div class="admin-action-card destructive">
          <div class="admin-action-info">
            <div class="admin-action-header">
              <v-icon icon="mdi-delete-forever" size="small" class="admin-action-icon"></v-icon>
              <span class="admin-action-title">Delete Entire Crate</span>
            </div>
            <p class="admin-action-desc">
              Permanently delete <strong>all versions</strong> of <code>{{ crateName }}</code>.
              This will break all crates that depend on it.
            </p>
          </div>
          <v-btn color="error" variant="flat" @click="handleDeleteCrate">
            <v-icon icon="mdi-delete-forever" size="small" class="me-2"></v-icon>
            Delete Crate
          </v-btn>
        </div>
      </div>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { crateService } from '../../services'
import { isSuccess } from '../../services/api'

// Props
const props = defineProps<{
  crateName: string
  version: string
}>()

// Router
const router = useRouter()

// Handle delete version
async function handleDeleteVersion() {
  if (!confirm(`Delete "${props.crateName}" version "${props.version}"?`)) return

  const result = await crateService.deleteVersion(props.crateName, props.version)
  if (isSuccess(result)) {
    router.push({ name: 'Crates' })
  } else {
    console.error('Failed to delete version:', result.error)
  }
}

// Handle delete crate
async function handleDeleteCrate() {
  if (!confirm(`Delete all versions of "${props.crateName}"?`)) return

  const result = await crateService.deleteCrate(props.crateName)
  if (isSuccess(result)) {
    router.push({ name: 'Crates' })
  } else {
    console.error('Failed to delete crate:', result.error)
  }
}
</script>

<style scoped>
/* Content Cards */
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px;
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
  border-radius: 8px;
}

.admin-alert-content {
  display: flex;
  align-items: flex-start;
}

.admin-alert-content .v-icon {
  flex-shrink: 0;
  margin-top: 2px;
}

.admin-link {
  color: rgb(var(--v-theme-primary));
  font-weight: 500;
  text-decoration: none;
}

.admin-link:hover {
  text-decoration: underline;
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
  border-radius: 8px;
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
  border-radius: 4px;
  font-size: 13px;
  font-family: 'Roboto Mono', monospace;
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
