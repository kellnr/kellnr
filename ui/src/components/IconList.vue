<template>
  <div class="iconList" v-if="isNotEmpty(list)">
    <div class="titleAndIcon">
      <i :class="icon"/>
      <span class="iconTitle">{{ title }}</span>
    </div>
    <div>
      <div v-for="item in list" :key="item">
        <div>
          <span class="iteratorIcon">
            <i class="fas fa-angle-right"/>
          </span>
          <span>{{ item }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  list: Array<string>
  icon: string
  title?: string
}>()

function isNotEmpty(obj: any) {
  return typeof obj !== "undefined" && obj !== null && obj.length > 0;
}
</script>

<style scoped>
.iteratorIcon {
  padding: 0.5rem;
  margin: 0.5rem 0rem 0.5rem 0rem;
}

.iconList {
  padding: 0.5rem;
  margin: 0.75rem 0 0.75rem 0;
  display: grid;
  grid-template-columns: minmax(8rem, max-content) auto;
}

.titleAndIcon {
  padding: 0 0.5rem 0 0;
  border-right-style: solid;
  border-width: 0.1rem;
  text-align: left;
}

.iconTitle {
  font-weight: bold;
  font-size: medium;
  padding-left: 0.5rem;
}
</style>
