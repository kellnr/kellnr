<template>
  <v-container>
    <v-card elevation="1" class="mb-4">
      <v-card-title class="text-h4">
        Rustdoc Queue
      </v-card-title>
      <v-card-text>
        <p>Current items in the queue for <i>rustdoc</i> auto-generation.</p>

        <v-alert v-if="emptyQueue" type="info" variant="tonal" class="mt-4">
          <i>Queue is empty</i>.
        </v-alert>
      </v-card-text>
    </v-card>

    <v-card v-if="!emptyQueue" elevation="1" class="mb-4 pa-4">
      <doc-queue-item-card v-for="(item, index) in queue" :key="item.name" :index="index + 1" :name="item.name"
        :version="item.version" class="mb-3"></doc-queue-item-card>
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
