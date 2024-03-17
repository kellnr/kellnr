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

@media only screen and (max-width: 768px) {
  .sidebarElement {
    padding: 0.5rem 0 0.5rem 0
  }
}

@media only screen and (min-width: 768px) {
  .sidebarElement {
    padding: 1rem 0 1rem 0
  }
}
</style>
