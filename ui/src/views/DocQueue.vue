<template>
  <v-container fluid class="pa-4">
    <!-- Header Card -->
    <v-card elevation="0" rounded="lg" class="doc-queue-card mb-4">
      <v-card-title class="section-header pa-4">
        <div class="d-flex align-center">
          <v-icon icon="mdi-file-document-multiple-outline" size="large" class="mr-3 header-icon" />
          <span class="text-h5 font-weight-bold">Rustdoc Queue</span>
        </div>
      </v-card-title>
      <v-card-text class="pa-4">
        <p class="description-text mb-0">
          Current items in the queue for <strong>rustdoc</strong> auto-generation.
          Documentation will be generated automatically for each crate in the queue.
        </p>
      </v-card-text>
    </v-card>

    <!-- Empty State -->
    <v-card v-if="emptyQueue" elevation="0" rounded="lg" class="empty-state-card pa-6 text-center">
      <v-icon icon="mdi-check-circle-outline" size="64" class="empty-icon mb-4" />
      <h3 class="text-h6 font-weight-medium mb-2">Queue is empty</h3>
      <p class="text-body-2 empty-description mb-0">
        All documentation has been generated. New crates will appear here when uploaded.
      </p>
    </v-card>

    <!-- Queue Items -->
    <v-card v-if="!emptyQueue" elevation="0" rounded="lg" class="queue-list-card">
      <v-card-title class="queue-header pa-4">
        <div class="d-flex align-center justify-space-between">
          <span class="text-subtitle-1 font-weight-medium">Pending Documentation</span>
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
import axios from "axios"
import { onMounted, onUnmounted, ref, computed } from "vue"
import type { DocQueueItem } from "../types/doc_queue_item"
import DocQueueItemCard from "../components/DocQueueItemCard.vue"
import { DOCS_QUEUE } from "../remote-routes"

const queue = ref<Array<DocQueueItem>>([])
const emptyQueue = computed(() => queue.value.length === 0)
let intervalId: ReturnType<typeof setInterval> | undefined

function getQueueItems() {
  axios.get(`${DOCS_QUEUE}?_=${Date.now()}`) // Use timestamp to avoid caching
    .then(response => {
      queue.value = response.data.queue ?? []
    })
    .catch(error => {
      console.log(error)
    })
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
.doc-queue-card {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
}

.section-header {
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.header-icon {
  color: rgb(var(--v-theme-primary));
}

.description-text {
  font-size: 0.95rem;
  line-height: 1.6;
  color: rgb(var(--v-theme-on-surface-variant));
}

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

.queue-header {
  background: rgba(var(--v-theme-primary), 0.03);
}
</style>
