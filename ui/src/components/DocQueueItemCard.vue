<template>
  <div id="docQueueItem" class="glass">
    <div id="left">
      <div class="index">{{ index }}.</div>
    </div>
    <div id="middle">
      <router-link class="link" :to="{name: 'Crate', query: {name: name, version: version}}">{{ name }}</router-link>
    </div>
    <div id="right">
      <div id="version">{{ version }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">

const props = defineProps<{
  index: number
  name: string
  version: string
}>()
</script>

<style scoped>
  #docQueueItem {
    display: grid;
    grid-template-columns: 1fr 4fr 4fr;
    text-align: left;
    border-bottom-width: 1px;
    margin: 0.5rem 0 0.5rem 0;
  }

  #left {
    grid-column: 1;
  }

  #middle {
    grid-column: 2;
  }

  #right {
    grid-column: 3;
  }
</style>