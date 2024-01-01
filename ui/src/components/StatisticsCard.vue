<template>
  <div class="glass card">
    <div class="top">
      <div class="left">{{ num }}</div>
      <div class="right">
        <span class="icon">
          <i :class="'fas ' + icon"></i>
        </span>
      </div>
    </div>
    <div class="bottom">{{ text }}</div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  num: number | string
  icon: string
  text: number | string 
}>()
</script>

<style scoped>
.card {
  display: grid;
  grid-template-columns: 1fr;
  grid-template-rows: 3fr 1fr;
  margin: 0.3rem 0.3rem 0.3rem 0.3rem;
}

body[color-theme="dark"] .card {
  color: var(--dark-color-white);
}

.top {
  grid-column: 1;
  grid-row: 1;
  display: grid;
  grid-template-columns: 2fr 1fr;
  grid-template-rows: 1fr;
}

.left {
  grid-column: 1;
  grid-row: 1;
  font-size: 2.5rem;
  display: flex;
  justify-content: center;
  align-items: center;
  min-width: 5rem;
}

.right {
  grid-column: 2;
  grid-row: 1;
  font-size: 1.5rem;
  display: flex;
  justify-content: center;
  align-items: center;
}

.bottom {
  grid-column: 1;
  grid-row: 2;
  text-align: center;
}
</style>

