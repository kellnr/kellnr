<template>
  <div id="searchTable">
    <div id="statSearch" class="glass">
      <div id="search">
        <input class="input is-info" v-model="searchText" v-on:keyup.enter="searchCrates(searchText)"
          placeholder="Search for crates" type="text"></input>
        <div id="cacheSwitch">
          <label class="inline-block-child switch">
            <input type="checkbox" v-model="cache" v-on:change="getCrates(0, page_size, true)" >
            <span class="slider round"></span>
          </label>
          <span id="cacheSwitchLabel" class="inline-block-child">
            Cache
          </span>
        </div>
      </div>
    </div>

    <div id="table" v-on:scroll="scrollHandler" ref="rtable">
      <div v-if="crates.length === 0" id="emptyTable">
        So empty...<br />
        To learn how to publish crates to <strong>Kellnr</strong>, read the <a href="https://kellnr.io/documentation"
          target="_blank">documentation</a>.
      </div>
      <template v-for="crate in crates" :key="crate">
        <crate-card class="cardview" :crate="crate.name" :version="crate.version" :updated="crate.date"
          :downloads="crate.total_downloads" :desc="crate.description" :doc-link="crate.documentation"></crate-card>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, onMounted, ref } from "vue"
import axios from "axios"
import CrateCard from "../components/CrateCard.vue"
import type { CrateOverview } from "../types/crate_overview";
import { CRATES, SEARCH } from "../remote-routes";
import { useRouter } from "vue-router";
import { login_required } from "../common/auth";

const crates = ref<Array<CrateOverview>>([])
const page = ref(0)
const total_num = ref(0)
const current_num = ref(0)
const page_size = ref(20)
const searchText = ref("")
const rtable = ref<HTMLDivElement | null>(null)
const router = useRouter()
const cache = ref(false)

onBeforeMount(() => {
  login_required()

  if (router.currentRoute.value.query.search) {
    searchText.value = router.currentRoute.value.query.search as string
    searchCrates(searchText.value)
  }
})

onMounted(() => {
  const cardHeight = 100; // Pixel size of "CrateCard" 18em * 14em. 1em = 16px
  const table = rtable.value;
  if (table) {
    const height = table.offsetHeight;
    const num_crates = Math.ceil(height / cardHeight);
    page_size.value = num_crates * 2;

    if (searchText.value === "") {
      getCrates(page.value, page_size.value, true);
    }
  }
})


function scrollHandler() {
  if (searchText.value !== "") {
    return;
  }
  let table = rtable.value;
  if (table) {
    let currentHeight = Math.ceil(table.offsetHeight + table.scrollTop);
    if (currentHeight >= table.scrollHeight) {
      getCrates(page.value, page_size.value);
    }
  }
}

function getCrates(next_page: number, page_size: number, clean: boolean = false) {
  if (current_num.value != 0 && total_num.value != null) {
    if (current_num.value >= total_num.value) {
      return;
    }
  }

  if (clean) {
    clearTable();
  }

  axios
    .get(CRATES, {
      params: { page: next_page, page_size: page_size, cache: cache.value },
    })
    .then((response) => {
      crates.value = crates.value.concat(response.data.crates);
      page.value = response.data.page + 1;
    });
}

function clearTable() {
  crates.value = [];
  page.value = 0;
}

function searchCrates(searchText: string) {
  // Safe in local variable as the search text
  // can change, while the functions runs
  const searchQuery = searchText;

  if (!searchQuery || searchQuery === "") {
    clearTable();
    getCrates(page.value, page_size.value);
    return;
  }

  axios
    .get(SEARCH, { params: { name: searchQuery, cache: cache.value } })
    .then((res) => {
      clearTable();
      crates.value = res.data.crates;
    })
    .catch((_error) => {
      clearTable();
      getCrates(page.value, page_size.value);
    });
}

</script>

<style scoped>
#searchTable {
  display: grid;
  grid-template-columns: 1fr;
  grid-template-rows: auto 1fr;
  height: 87vh;
}

#searchTable>#statSearch {
  grid-row: 1;
  padding-bottom: 0.5rem;
}

#searchTable>#table {
  grid-row: 2;
  overflow-x: hidden;
  padding: 15px 0 10px 0;
  text-align: center;
}

#searchTable>#emptyTable {
  text-align: center;
}

#statSearch {
  margin-bottom: 1rem;
}

.inline-block-child {
  display: inline-block;
}

#cacheSwitch {
  margin-top: 0.5rem;
}

#cacheSwitchLabel {
  margin-left: 0.5rem;
}
</style>
