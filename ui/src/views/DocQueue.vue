<template>
  <v-container fluid class="pa-0">
    <PageHeader
      title="Doc queue"
      subtitle="Crates waiting for rustdoc auto-generation. Kellnr builds the documentation for each crate in the queue automatically."
    />

    <!-- Empty State -->
    <v-card v-if="emptyQueue" elevation="0" class="empty-state-card pa-6 text-center">
      <v-icon icon="mdi-check-circle-outline" size="64" class="empty-icon mb-4" />
      <h2 class="text-title-large font-weight-medium mb-2">Queue is empty</h2>
      <p class="text-body-medium empty-description mb-0">
        All documentation has been generated. New crates will appear here when uploaded.
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
  </v-container>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from "vue"
import type { DocQueueItem } from "../types/doc_queue_item"
import DocQueueItemCard from "../components/DocQueueItemCard.vue"
import { PageHeader } from "../components/common"
import { crateService } from "../services"
import { isSuccess } from "../services/api"

const queue = ref<Array<DocQueueItem>>([])
const emptyQueue = computed(() => queue.value.length === 0)
let intervalId: ReturnType<typeof setInterval> | undefined

async function getQueueItems() {
  const result = await crateService.getDocsQueue()
  if (isSuccess(result)) {
    queue.value = result.data ?? []
  }
}

onMounted(() => {
  getQueueItems()
  intervalId = setInterval(() => {
    getQueueItems()
  }, 3000)
})

onUnmounted(() => {
  if (intervalId) clearInterval(intervalId)
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
