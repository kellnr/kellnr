<template>
<div class="configItem">
  <span class="headerToml">default.toml</span>
  <span class="headerEnv">env. var.</span>
  <span class="headerValue">value</span>
  <span class="nameToml">{{props.toml}}</span>
  <span class="nameEnv">{{props.env}}</span>
  <span class="value">{{props.value}}</span>
</div>
</template>

<script setup lang="ts">
const props = defineProps<{
  toml: string
  env: string
  value: string | number | boolean
}>()
</script>

<style scoped>
.configItem {
  border-bottom-style: solid;
  border-bottom-width: 0.01rem;
  margin: 0.5rem 0rem 0.5rem 0rem;
  padding: 0.5rem;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
  grid-template-rows: auto auto;
}

.headerToml {
  grid-column: 1;
  grid-row: 1;
  font-size: smaller;
}

.headerEnv {
  grid-column: 2;
  grid-row: 1;
  font-size: smaller;
}

.headerValue {
  grid-column: 3;
  grid-row: 1;
  font-size: smaller;
}

.nameToml {
  grid-column: 1;
  grid-row: 2;
  font-weight: bolder;
}

.nameEnv {
  grid-column: 2;
  grid-row: 2;
  font-weight: bolder;
}

.value {
  grid-column: 3;
  grid-row: 2;
  font-weight: bolder;
}
</style>
