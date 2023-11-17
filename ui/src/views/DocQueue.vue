<template>
  <h1 class="k-h1">Rustdoc Queue</h1>
  <p>Current items in the queue for <i>rustdoc</i> auto-generation.</p>
  <div v-if="emptyQueue" id="emptyDocQueue">
    <p>
      <i>Queue is empty</i>.
    </p>
  </div>
  <div id="docQueueItems">
    <template v-for="(item, index) in queue" :key="name">
      <doc-queue-item-card :index="index+1" :name="item.name" :version="item.version"></doc-queue-item-card>
    </template>
  </div>
</template>

<script setup lang="ts">
import axios from "axios"
import {onMounted, ref} from "vue"
import {DocQueueItem} from "../types/doc_queue_item"
import DocQueueItemCard from "../components/DocQueueItemCard.vue"
import {DOCS_QUEUE, kellnr_url} from "../remote-routes";

const queue = ref<Array<DocQueueItem>>()
const emptyQueue = ref(false)

function getQueueItems() {
  axios
      .get(DOCS_QUEUE)
      .then(response => {
        queue.value = response.data.queue;
        emptyQueue.value = queue.value?.length === 0;
      })
      .catch(error => {
        console.log(error)
      })
}

onMounted(() => {
  getQueueItems()
})

</script>

<style scoped>
#docQueueItems {
  padding-top: 1rem;
}

#emptyDocQueue {
  margin-top: 1rem;
}
</style>