<template>
  <v-card class="mb-4 content-card" elevation="0" data-testid="crate-versions">
    <v-card-text>
      <v-alert v-if="errorMessage" type="error" variant="tonal" closable class="mb-4" data-testid="versions-error"
        @click:close="errorMessage = ''">
        {{ errorMessage }}
      </v-alert>

      <!-- Versions are grouped by semver compatibility, like 1.x or 0.3.x -->
      <section v-for="group in versionGroups" :key="group.label" class="version-group" data-testid="version-group">
        <h3 class="version-group-label" data-testid="version-group-label">
          {{ group.label }}
          <span class="version-group-count">{{ group.versions.length }}</span>
        </h3>
        <Version v-for="version in group.versions" :key="version.version" :name="crateName" :version="version"
          :can-manage="canManage" :busy="busyVersion === version.version" @toggle-yank="handleToggleYank" />
      </section>
    </v-card-text>

    <ConfirmDialog v-model="yankDialogOpen" title="Yank version"
      :message="`Yank ${crateName} version ${pendingYank?.version ?? ''}?`"
      sub-message="Cargo no longer selects a yanked version for new lockfiles. Existing lockfiles keep working, and you can undo the yank."
      confirm-text="Yank" confirm-icon="mdi-tag-arrow-down-outline" @confirm="confirmYank" />
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import Version from "../Version.vue";
import { ConfirmDialog } from "../common";
import { crateService } from "../../services";
import { isSuccess } from "../../services/api";
import { versionGroup } from "../../common/format";
import type { CrateVersionData } from "../../types/crate_data";

const props = defineProps<{
  crateName: string
  versions: CrateVersionData[]
  canManage: boolean
}>();

const emit = defineEmits<{
  changed: []
}>();

const yankDialogOpen = ref(false);
const pendingYank = ref<CrateVersionData | null>(null);
const busyVersion = ref<string | null>(null);
const errorMessage = ref("");

// The versions are sorted newest first, so are the groups
const versionGroups = computed(() => {
  const groups: { label: string, versions: CrateVersionData[] }[] = [];
  for (const version of props.versions) {
    const label = versionGroup(version.version);
    const group = groups.find(g => g.label === label);
    if (group) {
      group.versions.push(version);
    } else {
      groups.push({ label, versions: [version] });
    }
  }
  return groups;
});

// Unyanking only makes a version available again, so it needs no confirmation
function handleToggleYank(version: CrateVersionData) {
  if (version.yanked) {
    changeYank(version, false);
    return;
  }
  pendingYank.value = version;
  yankDialogOpen.value = true;
}

function confirmYank() {
  if (pendingYank.value) {
    changeYank(pendingYank.value, true);
  }
}

async function changeYank(version: CrateVersionData, yank: boolean) {
  errorMessage.value = "";
  busyVersion.value = version.version;
  const result = yank
    ? await crateService.yankVersion(props.crateName, version.version)
    : await crateService.unyankVersion(props.crateName, version.version);
  busyVersion.value = null;
  if (isSuccess(result)) {
    emit("changed");
  } else {
    errorMessage.value = `Failed to ${yank ? "yank" : "unyank"} version ${version.version}: ${result.error.message}`;
  }
}
</script>

<style scoped>
.content-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: var(--kellnr-radius-card);
  overflow: hidden;
}

.version-group + .version-group {
  margin-top: 20px;
}

.version-group-label {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
  font-family: var(--kellnr-font-mono);
  color: rgb(var(--v-theme-on-surface));
}

.version-group-count {
  font-family: var(--kellnr-font-sans);
  font-size: 11px;
  font-weight: 500;
  padding: 1px 8px;
  border-radius: 9999px;
  background: rgb(var(--v-theme-surface-variant));
  color: rgb(var(--v-theme-on-surface-variant));
}
</style>
