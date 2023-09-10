<template>
  <div class="sidebarElement">
    <div class="sidebarHeader">
      <span class="icon sideHeaderIcon"><i :class="'fas ' + icon"></i></span>
      <span class="sideHeaderTitle">{{ props.header }}</span>
    </div>

    <div class="sidebarContent">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  icon: string
  header: string
}>()
</script>

<style>
.sidebarElement {
  padding: 1rem 0 1rem 0
}

.sidebarHeader {
  display: grid;
  grid-template-columns: auto 1fr;
}

.sideHeaderIcon {
}

.sideHeaderTitle {
  padding: 0 0 0.3rem 0.5rem;
}

.sidebarContent {
  font-size: larger;
}
</style>
