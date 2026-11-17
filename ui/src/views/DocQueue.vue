<template>
  <v-container fluid class="pa-0">
    <PageHeader
      title="Doc queue"
      subtitle="Crates waiting for rustdoc auto-generation and the latest finished builds. Kellnr builds the documentation for each crate in the queue automatically."
    />

    <!-- Empty State -->
    <v-card v-if="emptyQueue" elevation="0" class="empty-state-card pa-6 text-center">
      <v-icon icon="mdi-check-circle-outline" size="64" class="empty-icon mb-4" />
      <h2 class="text-title-large font-weight-medium mb-2">Queue is empty</h2>
      <p class="text-body-medium empty-description mb-0">
        No crates are waiting for documentation. New crates will appear here when uploaded.
      </p>
    </v-card>

    <!-- Queue Items -->
    <v-card v-if="!emptyQueue" elevation="0" class="queue-list-card">
      <v-card-title class="queue-header pa-4">
        <div class="d-flex align-center justify-space-between">
          <h2 class="queue-title text-body-large font-weight-medium">Pending documentation</h2>
          <v-chip size="small" variant="tonal" color="primary">
            {{ queue.length }} item{{ queue.length !== 1 ? 's' : '' }}
          </v-chip>
        </div>
      </v-card-title>
      <v-divider />
      <v-card-text class="pa-0">
        <doc-queue-item-card
          v-for="(item, index) in queue"
          :key="item.name"
          :index="index + 1"
          :name="item.name"
          :version="item.version"
          :is-last="index === queue.length - 1"
        />
      </v-card-text>
    </v-card>

    <!-- Recent Builds -->
    <v-card v-if="builds.length > 0" elevation="0" class="queue-list-card mt-6" data-testid="doc-builds">
      <v-card-title class="queue-header pa-4">
        <h2 class="queue-title text-body-large font-weight-medium">Recent builds</h2>
      </v-card-title>
      <v-divider />
      <v-card-text class="pa-0">
        <doc-build-item-card
          v-for="(build, index) in builds"
          :key="build.id"
          :build="build"
          :is-last="index === builds.length - 1"
          :rebuilding="rebuilding === buildKey(build)"
          :queued="queuedKeys.has(buildKey(build)) || !newestBuildIds.has(build.id)"
          @rebuild="rebuild"
        />
      </v-card-text>
    </v-card>
  </v-container>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from "vue"
import type { DocQueueItem } from "../types/doc_queue_item"
import type { DocBuild } from "../types/doc_build"
import DocQueueItemCard from "../components/DocQueueItemCard.vue"
import DocBuildItemCard from "../components/DocBuildItemCard.vue"
import { PageHeader } from "../components/common"
import { crateService } from "../services"
import { isSuccess } from "../services/api"
import { useNotification } from "../composables"

const queue = ref<Array<DocQueueItem>>([])
const builds = ref<Array<DocBuild>>([])
const rebuilding = ref<string | null>(null)
const emptyQueue = computed(() => queue.value.length === 0)
const queuedKeys = computed(() => new Set(queue.value.map(buildKey)))
// Only the newest build of a version offers a rebuild.
const newestBuildIds = computed(() => {
  const seen = new Set<string>()
  const ids = new Set<number>()
  for (const build of builds.value) {
    if (!seen.has(buildKey(build))) {
      seen.add(buildKey(build))
      ids.add(build.id)
    }
  }
  return ids
})
const notification = useNotification()
const QUEUE_POLL_MS = 3000
// Catches builds that were queued and finished between two queue polls.
const BUILDS_POLL_MS = 30000
let queueIntervalId: ReturnType<typeof setInterval> | undefined
let buildsIntervalId: ReturnType<typeof setInterval> | undefined

async function getQueueItems() {
  const result = await crateService.getDocsQueue()
  if (!isSuccess(result)) {
    return
  }
  const previousKeys = queue.value.map(buildKey)
  queue.value = result.data ?? []
  // An entry that left the queue is a finished build.
  if (previousKeys.some((key) => !queuedKeys.value.has(key))) {
    await getRecentBuilds()
  }
}

// Loaded only when a build may have finished, as checking the access to
// every listed crate is more expensive than reading the queue.
async function getRecentBuilds() {
  const result = await crateService.getRecentDocBuilds()
  if (isSuccess(result)) {
    builds.value = result.data ?? []
  }
}

// The queue lists crate names in lowercase, builds with their original name.
function buildKey(build: { name: string; version: string }): string {
  return `${build.name.toLowerCase()}@${build.version}`
}

async function rebuild(build: DocBuild) {
  rebuilding.value = buildKey(build)
  const result = await crateService.buildDocs(build.name, build.version)
  rebuilding.value = null
  if (isSuccess(result)) {
    notification.showSuccess(`Queued the docs build of ${build.name} ${build.version}`)
    await getQueueItems()
  } else {
    notification.showError(`Failed to queue the docs build of ${build.name} ${build.version}: ${result.error?.message}`)
  }
}

onMounted(() => {
  getQueueItems()
  getRecentBuilds()
  queueIntervalId = setInterval(getQueueItems, QUEUE_POLL_MS)
  buildsIntervalId = setInterval(getRecentBuilds, BUILDS_POLL_MS)
})

onUnmounted(() => {
  clearInterval(queueIntervalId)
  clearInterval(buildsIntervalId)
})
</script>

<style scoped>
/* Empty State */
.empty-state-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.empty-icon {
  color: rgb(var(--v-theme-primary));
  opacity: 0.6;
}

.empty-description {
  color: rgb(var(--v-theme-on-surface-variant));
  max-width: 400px;
  margin: 0 auto;
}

/* Queue List */
.queue-list-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.queue-title {
  margin: 0;
}

.queue-header {
  background: rgba(var(--v-theme-primary), 0.03);
}
</style>
