<template>
    <div id="searchTable">
      <div id="statSearch" class="glass">
        <div id="search">
          <div class="field">
            <div class="control">
              <input
                  class="input is-info"
                  v-model="searchText"
                  v-on:keyup="searchCrates()"
                  placeholder="Search for crates"
                  type="text"
              />
            </div>
          </div>
        </div>
      </div>

      <div id="table" v-on:scroll="scrollHandler" ref="rtable">
        <div v-if="emptyCrates" id="emptyTable">
          So empty...<br/>
          To learn how to publish crates to <strong>Kellnr</strong>, read the <a href="https://kellnr.io/documentation" target="_blank">documentation</a>.
        </div>
        <template v-for="crate in crates" :key="crate">
          <crate-card
              class="cardview"
              :crate="crate.original_name"
              :version="crate.max_version"
              :updated="crate.last_updated"
              :downloads="crate.total_downloads"
              :desc="crate.description"
              :doc-link="crate.documentation"
          ></crate-card>
        </template>
      </div>
    </div>
</template>

<script setup lang="ts">
import {onBeforeMount, onMounted, ref} from "vue"
import axios from "axios"
import CrateCard from "../components/CrateCard.vue"
import {CrateOverview} from "../types/crate_overview";
import {CRATES, SEARCH, VERSION} from "../remote-routes";
import {store} from "../store/store";
import {useRouter, useRoute} from "vue-router";

const crates = ref<Array<CrateOverview>>([])
const emptyCrates = ref(false)
const page = ref(0)
const init_page_size = ref(0) // Set dynamically in mounted based on available space
const timer = ref<number | null>(0)
const total_num = ref(0)
const current_num = ref(0)
const page_size = ref(20)
const searchText = ref("")
const search_key_timeout = ref(500)
const rtable = ref<HTMLDivElement | null>(null)
const router = useRouter()

onBeforeMount(() => {
  login_required()

  if(router.currentRoute.value.query.search) {
    searchText.value = router.currentRoute.value.query.search as string
    searchCrates()
  }
})

function login_required() {
  if(store.state.loggedIn === false) {
    // Check if authentication is required
    // to view crates. -> "auth_required = true" in Kellnr settings.
    
    axios.get(VERSION).then((_response) => {
      // do nothing -> no auth required
    }).catch((error) => {
      if(error.response.status === 401) {
        router.push("/login")
      }
    })
  } 
}

function scrollHandler() {
  let table = rtable.value;
  if (table) {
    let currentHeight = Math.ceil(table.offsetHeight + table.scrollTop);
    if (currentHeight >= table.scrollHeight) {
      getCrates(page_size.value);
    }
  }
}

function getCrates(page_size: number) {
  if (current_num.value != 0 && total_num.value != null) {
    if (current_num.value >= total_num.value) {
      return;
    }
  }

  axios
    .get(CRATES, {
      params: {page: page.value, page_size: page_size},
    })
    .then((response) => {
      crates.value = crates.value.concat(response.data.crates);
      page.value = page.value + 1;
      total_num.value = response.data.total_num;
      current_num.value = response.data.current_num;
      emptyCrates.value = crates.value.length === 0;
    });
}

function clear_table() {
  crates.value = [];
  page.value = 0;
  total_num.value = 0;
  current_num.value = 0;
}

function searchCrates() {
  // Safe in local variable as the search text
  // can change, while the functions runs
  const searchQuery = searchText.value;

  if (!searchQuery || searchQuery === "") {
    clear_table();
    getCrates(init_page_size.value);
    return;
  }

  if (timer.value) {
    clearTimeout(timer.value);
    timer.value = null;
  }
  timer.value = setTimeout(() => {
    axios
        .get(SEARCH, {params: {name: searchQuery}})
        .then((res) => {
          clear_table();
          crates.value = res.data.crates;
          total_num.value = res.data.total_num;
          current_num.value = res.data.current_num;
        })
        .catch((_error) => {
          clear_table();
          getCrates(init_page_size.value);
        });
  }, search_key_timeout.value);
}

onMounted(() => {
  const cardHeight = 100; // Pixel size of "CrateCard" 18em * 14em. 1em = 16px
  const table = rtable.value;
  if (table) {
    const height = table.offsetHeight;
    const num_crates = Math.ceil(height / cardHeight);
    init_page_size.value = num_crates * 2;

    if (searchText.value === "") {
      getCrates(init_page_size.value);
    }
  }
})

</script>

<style scoped>
#searchTable {
  display: grid;
  grid-template-columns: 1fr;
  grid-template-rows: auto 1fr;
  height: 87vh;
}

#searchTable > #statSearch {
  grid-row: 1;
  padding-bottom: 0.5rem;
}

#searchTable > #table {
  grid-row: 2;
  overflow-x: hidden;
  padding: 15px 0 10px 0;
  text-align: center;
}

#searchTable > #emptyTable {
  text-align: center;
}

#statSearch {
  margin-bottom: 1rem;
}
</style>
