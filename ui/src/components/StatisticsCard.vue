<template>
  <div class="statCard">
    <div class="left">
      <span v-bind:style="{color: iconColor}" class="icon">
        <i :class="'fas ' + icon"></i>
      </span>
    </div>
    <div class="right">
      <span class="text"><slot></slot></span>
      <span class="num">{{ num }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  num: number
  icon: string
  iconColor?: string
}>()
</script>

<style scoped>
.statCard {
  color: black;
  margin: 0 0 0.3em 0;
  width: 60%;

  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-rows: auto;
}

.statCard .left {
  grid-column: 1;
  grid-row: 1;
  display: grid;
  grid-template-rows: auto;
  place-items: center;
}

.statCard .icon {
  grid-column: 1;
  grid-row: 1;
  margin-right: 0.3em;
  margin-left: 0.5em;
}

.statCard .right {
  grid-column: 2;
  grid-row: 1;
  display: grid;
  grid-template-columns: 3fr 1fr;
}

.statCard .right .num {
  grid-column: 2;
  text-align: right;
}

.statCard .right .text {
  grid-column: 1;
}

body[color-theme="dark"] .statCard {
  color: var(--dark-color-white);
}
</style>
