<template>
  <div class="configItem">
    <span class="tomlValue">
      <div>[{{ props.tomlTable }}]</div>
      <div>{{ props.toml }}</div>
    </span>
    <span class="nameEnv">{{ props.env }}</span>
    <span class="value">{{ props.value }}</span>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  tomlTable: string
  toml: string
  env: string
  value: string | number | boolean
}>()
</script>

<style scoped>
.configItem {
  border-bottom-style: solid;
  border-bottom-width: 0.01rem;
  padding: 0.5rem 0 0.5rem 0;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
}

.tomlValue {
  grid-column: 1;
}

.nameEnv {
  grid-column: 2;
}

.value {
  grid-column: 3;
}
</style>
