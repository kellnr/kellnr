<template>
  <div class="k-footer">
      <strong>kellnr</strong> &copy; by
      <a class="link" href="https://www.kellnr.io">kellnr.io</a> - <a class="link" href="https://kellnr.io/changelog">Kellnr {{ versionInfo.version }}</a>
  </div>
</template>

<script setup lang="ts">
import {onBeforeMount, ref} from "vue";
import axios from "axios";
import {defaultVersionInfo, VersionInfo} from "../types/version_info";
import {VERSION} from "../remote-routes";

const versionInfo = ref(defaultVersionInfo())

onBeforeMount(() => {
  get_version()
})

function get_version() {
  axios.get(VERSION).then((response) => {
    versionInfo.value = response.data as VersionInfo;
  });
}
</script>

<style scoped>

.k-footer {
  padding-top: 0.5rem;
  padding-bottom: 1rem;
  text-align: center;
  font-size: 0.8rem;
}

body[color-theme="dark"] strong {
  color: var(--dark-color-white);
}

</style>
