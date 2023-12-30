<template>
  <div id="statistics">
    <div class="statisticsCards" v-if="statistics">
      <!-- Hosted crates statistics -->
      <statistics-card :num="statistics.num_crates" :icon="'fa-boxes'" :text="'Crates'"></statistics-card>
      <statistics-card :num="statistics.num_crate_versions" :icon="'fa-code-branch'" :text="'Versions'"></statistics-card>
      <statistics-card :num="statistics.num_crate_downloads" :icon="'fa-cloud-download-alt'"
        :text="'Downloads'"></statistics-card>

      <!-- Top three crates -->
      <statistics-card v-if="statistics.top_crates.first[1] > 0" :num="statistics.top_crates.first[1]" :icon="'fa-medal'"
        :text="statistics.top_crates.first[0]"></statistics-card>
      <statistics-card v-if="statistics.top_crates.second[1] > 0" :num="statistics.top_crates.second[1]" :icon="'fa-medal'"
        :text="statistics.top_crates.second[0]"></statistics-card>
      <statistics-card v-if="statistics.top_crates.third[1] > 0" :num="statistics.top_crates.third[1]" :icon="'fa-medal'"
        :text="statistics.top_crates.third[0]"></statistics-card>

      <!-- Last updated crate -->
      <statistics-card v-if="statistics.last_updated_crate" :num="statistics.last_updated_crate[0]" :icon="'fa-calendar'"
        :text="'Last Updated ' + statistics.last_updated_crate[1]"></statistics-card>

      <!-- Proxy statistics - displayed only if proxy is enabled -->
      <statistics-card v-if="statistics.proxy_enabled" :num="statistics.num_proxy_crates" :icon="'fa-boxes'"
        :text="'Proxy Crates'"></statistics-card>
      <statistics-card v-if="statistics.proxy_enabled" :num="statistics.num_proxy_crate_versions" :icon="'fa-code-branch'"
        :text="'Proxy Versions'"></statistics-card>
      <statistics-card v-if="statistics.proxy_enabled" :num="statistics.num_proxy_crate_downloads"
        :icon="'fa-cloud-download-alt'" :text="'Proxy Downloads'"></statistics-card>
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
