<template>
  <div id="statistics">
    <h1>Statistics</h1>
    <div class="statisticsCards">
      <statistics-card2 :num="statistics.num_crates" :icon="'fa-boxes'" :text="'number of crates'"></statistics-card2>
      <statistics-card2 :num="statistics.num_crate_versions" :icon="'fa-boxes'" :text="'number of versions'"></statistics-card2>
      <statistics-card2 :num="statistics.num_crate_downloads" :icon="'fa-boxes'" :text="'number of downloads'"></statistics-card2>
      <statistics-card2 :num="statistics.num_proxy_crates" :icon="'fa-boxes'" :text="'number of proxy crates'"></statistics-card2>
      <statistics-card2 :num="statistics.num_proxy_crate_versions" :icon="'fa-boxes'" :text="'number of proxy versions'"></statistics-card2>
      <statistics-card2 :num="statistics.num_proxy_crate_downloads" :icon="'fa-boxes'" :text="'number of proxy downloads'"></statistics-card2>
    </div>
  </div>
</template>

<script setup lang="ts">
import axios from 'axios';
import {onBeforeMount, ref} from "vue";
import { STATISTICS } from '../remote-routes';
import StatisticsCard2 from '../components/StatisticsCard2.vue';
import { Statistics} from '../types/statistics';

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
}
</style>
