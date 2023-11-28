<template>
  <div class="configHeader">
    <span class="headerToml">default.toml</span>
    <span class="headerEnv">env. var.</span>
    <span class="headerValue">value</span>
  </div>
</template>

<style scoped>
.configHeader {
  border-bottom-style: solid;
  border-bottom-width: 0.02rem;
  padding: 0.5rem 0 0 0;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
}

.headerToml {
  grid-column: 1;
  margin-bottom: 0.1rem;
  font-weight: bold;
}

.headerEnv {
  grid-column: 2;
  margin-bottom: 0.1rem;
  font-weight: bold;
}

.headerValue {
  grid-column: 3;
  margin-bottom: 0.1rem;
  font-weight: bold;
}
</style>
