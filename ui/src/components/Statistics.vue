<template>
  <div id="statsContent" class=" wrapper">
    <statistics-card
        id="uniqueCrates"
        :num="uniqueCrates"
        :icon="'fa-boxes'"
    >Crates
    </statistics-card>
    <statistics-card
        id="crateVersions"
        :num="crateVersions"
        :icon="'fa-code-branch'"
    >Versions
    </statistics-card>
    <statistics-card
        id="downloads"
        :num="downloads"
        :icon="'fa-cloud-download-alt'"
    >Downloads
    </statistics-card>

    <statistics-card
        v-if="top1.trunc_name"
        id="top1"
        :num=top1.downloads
        :icon="'fa-medal'"
        :icon-color="'#C9B037'"
    >
      <router-link class="clickable" :to="{name: 'Crate', query: {name: top1.orig_name}}">{{ top1.trunc_name }}</router-link>
    </statistics-card>
    <statistics-card
        v-if="top2.trunc_name"
        id="top2"
        :num=top2.downloads
        :icon="'fa-medal'"
        :icon-color="'#B4B4B4'"
    >
      <router-link class="clickable" :to="{name: 'Crate', query: {name: top2.orig_name}}">{{ top2.trunc_name }}</router-link>
    </statistics-card>
    <statistics-card
        v-if="top3.trunc_name"
        id="top3"
        :num=top3.downloads
        :icon="'fa-medal'"
        :icon-color="'#AD8A56'"
    >
      <router-link class="clickable" :to="{name: 'Crate', query: {name: top3.orig_name}}">{{ top3.trunc_name }}</router-link>
    </statistics-card>
  </div>
</template>

<script setup lang="ts">
import StatisticsCard from "../components/StatisticsCard.vue";
import {onBeforeMount, ref} from "vue";
import axios from "axios";
import {useRouter} from "vue-router";
import {STATISTICS} from "../remote-routes";

type Stat = {
  trunc_name: string
  orig_name: string
  downloads: number
}

const emptyStat = {
  trunc_name: "",
  orig_name: "",
  downloads: 0
}

const uniqueCrates = ref(0)
const crateVersions = ref(0)
const downloads = ref(0)
const top1 = ref<Stat>(emptyStat)
const top2 = ref<Stat>(emptyStat)
const top3 = ref<Stat>(emptyStat)
const router = useRouter()

onBeforeMount(() => {
  getStats()
})

function getStats() {
  const trunc = 15
  axios
      .get(STATISTICS)
      .then((res) => {
        uniqueCrates.value = res.data.unique_crates;
        crateVersions.value = res.data.crate_versions;
        downloads.value = res.data.downloads;
        top1.value = {
          orig_name: res.data.top1[0],
          trunc_name: truncate(res.data.top1[0], trunc),
          downloads: res.data.top1[1]
        }
        top2.value = {
          orig_name: res.data.top2[0],
          trunc_name: truncate(res.data.top2[0], trunc),
          downloads: res.data.top2[1]
        }
        top3.value = {
          orig_name: res.data.top3[0],
          trunc_name: truncate(res.data.top3[0], trunc),
          downloads: res.data.top3[1]
        }
      })
      .catch((error) => {
        console.log(error);
      });
}

function truncate(value: string, length: number) {
  if (value == undefined) {
    return "";
  }
  if (value.length > length) {
    return value.substring(0, length) + "...";
  } else {
    return value;
  }
}
</script>

<style scoped>

.wrapper {
  height: fit-content;
}

#statsContent {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  grid-template-rows: 1fr 1fr;
  place-items: center;
}

#uniqueCrates {
  grid-row: 1;
  grid-column: 1;
}

#crateVersions {
  grid-row: 1;
  grid-column: 2;
}

#downloads {
  grid-row: 1;
  grid-column: 3;
}

#top1 {
  grid-row: 2;
  grid-column: 1;
}

#top2 {
  grid-row: 2;
  grid-column: 2;
}

#top3 {
  grid-row: 2;
  grid-column: 3;
}

</style>
