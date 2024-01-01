<template>
  <div id="searchBar" class="glass">
    <input class="input is-info" v-model="searchText" v-on:keyup.enter="searchCrates()" placeholder="Search for crates"
      type="text" />
  </div>

  <div id="center">

    <p id="welcome-text">
      Welcome to Kellnr, your private crate registry. To get started, have a look at the <a
        href="https://kellnr.io/documentation">documentation</a>.
    </p>

    <h1 id="overview" class="k-h1">Overview</h1>
  </div>

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
      <statistics-card v-if="statistics.top_crates.second[1] > 0" :num="statistics.top_crates.second[1]"
        :icon="'fa-medal'" :text="statistics.top_crates.second[0]"></statistics-card>
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
import router from '../router';

const statistics = ref<Statistics>();
const searchText = ref("");

onBeforeMount(() => {
  axios.get(STATISTICS).then((response) => {
    statistics.value = response.data;
  });
});

function searchCrates() {
  if (searchText.value.length > 0) {
    router.push({ path: '/crates', query: { search: searchText.value } });
  }
}
</script>

<style scoped>
.statisticsCards {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
}

#searchBar {
  margin-bottom: 2rem;
}

#center {
  text-align: center;
}

#welcome-text {
  margin-bottom: 1.5rem;
}
</style>
