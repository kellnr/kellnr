<template>
  <div class="iconElement">
    <div class="titleAndIcon">
      <i :class="icon"/>
      <span class="iconTitle"> {{ title }}</span>
    </div>
    <div class="content">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  icon: string
  title: string
}>()
</script>

<style scoped>
.iconElement {
  padding: 0.5rem;
  margin: 0.75rem 0rem 0.75rem 0rem;
  display: grid;
  grid-template-columns: minmax(8rem, max-content) auto;
}

.titleAndIcon {
  padding: 0 0.5rem 0 0;
  border-right-style: solid;
  border-width: 0.1rem;
  text-align: left;
  font-weight: bold;
}

.iconTitle {
  padding-left: 0.5rem;
}

.content {
  padding: 0 0 0 0.5rem;
}

</style>
