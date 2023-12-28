<template>
  <div id="statistics">
    <h1>Statistics</h1>
    <div class="statisticsCards" v-if="statistics">
      <statistics-card :num="statistics.num_crates" :icon="'fa-boxes'" :text="'Crates'"></statistics-card>
      <statistics-card :num="statistics.num_crate_versions" :icon="'fa-code-branch'" :text="'Versions'"></statistics-card>
      <statistics-card :num="statistics.num_crate_downloads" :icon="'fa-cloud-download-alt'"
        :text="'Downloads'"></statistics-card>
      <statistics-card :num="statistics.num_proxy_crates" :icon="'fa-boxes'" :text="'Proxy Crates'"></statistics-card>
      <statistics-card :num="statistics.num_proxy_crate_versions" :icon="'fa-code-branch'"
        :text="'Proxy Versions'"></statistics-card>
      <statistics-card :num="statistics.num_proxy_crate_downloads" :icon="'fa-cloud-download-alt'"
        :text="'Proxy Downloads'"></statistics-card>
    </div>
  </div>
</template>

<script setup lang="ts">
import axios from 'axios';
import { onBeforeMount, ref } from "vue";
import { STATISTICS } from '../remote-routes';
import StatisticsCard from '../components/StatisticsCard.vue';
import { Statistics } from '../types/statistics';

const statistics = ref<Statistics>();


onBeforeMount(() => {
  axios.get(STATISTICS).then((response) => {
    statistics.value = response.data;
  });
});
</script>

<style scoped>
.statisticsCards {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
}
</style>
