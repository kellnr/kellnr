<template>
  <div class="crateCard glass">
      <div class="crateTitle">
        <router-link class="crateName" :to="{name: 'Crate', query: {name: crate, version: version}}">{{crate}}</router-link>
        <div class="crateVersion">v{{ version }}</div>
      </div>
      <div class="crateDesc">
        {{ desc || "no description available" }}
      </div>
    <div class="crateStatistics">
      <div class="crateIconInfo">
        <span class="crateIcon"><i class="fas fa-cloud-download-alt"></i></span>
        <span><span class="mobile-invisible">Downloads: </span>{{ downloads }}</span>
      </div>
      <div class="crateIconInfo">
        <span class="crateIcon"><i class="fas fa-calendar-alt"></i></span>
        <span><span class="mobile-invisible">Updated: </span>{{ humanizedLastUpdated }}</span>
      </div>
      <div class="crateIconInfo">
        <span class="crateIcon"><i class="fa-solid fa-book"></i></span>
        <a v-if="docLink" v-bind:href="docLink" class="clickable" target="_blank">Documentation</a>
        <router-link v-if="!docLink" class="clickable" to="/publishdocs">Add Documentation</router-link>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
//"copyToCb(crate + ' = &quot;' + version + '&quot;')

import {computed} from "vue";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";

dayjs.extend(relativeTime);

const props = defineProps<{
  crate: string
  desc?: string
  downloads: number
  version: string
  updated: string
  docLink?: string
}>()

const humanizedLastUpdated = computed(() => {
  return dayjs(props.updated).fromNow();
})

function copyToCb(text: string) {
  navigator.clipboard.writeText(text);
}
</script>

<style scoped>
.crateCard {
  display: grid;
  text-align: left;
  margin-bottom: 1rem;
}

.crateTitle {
  grid-area: title;
  display: inline-flex;
}

.crateName {
  font-weight: bolder;
  font-size: larger;
  color: var(--color-darkest);
}

.crateName:hover {
  color: var(--color-dark);
}

body[color-theme="dark"] .crateName {
  color: var(--dark-color-middle);
}

body[color-theme="dark"] .crateName:hover {
  color: var(--dark-color-white);
}

.crateVersion {
  font-size: medium;
  padding-top: 0.188rem;
  margin-left: 0.5rem;
}

.crateDesc {
  grid-area: description;
  /* autoprefixer: off */
  -webkit-box-orient: vertical;
  /* autoprefixer: on */
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
}

.crateStatistics {
  grid-area: crateStatistics;
  display: flex;
}

.crateIconInfo {
  display: grid;
  grid-template-columns: minmax(2rem, max-content) auto;
}

.crateIcon {
  text-align: center;
}

.boxIcon {
  font-size: 2.5rem;
  text-align: center;
  margin-right: 1.5rem;
  margin-left: 1rem;
}

.boxIcon > i {
  color: var(--color-darkest);
}

@media only screen and (max-width: 768px) {
  .crateCard {
    grid-template-rows: auto auto auto;
    grid-template-areas: 
      "title"
      "description"
      "crateStatistics";
  }
  .crateDesc {
    -webkit-line-clamp: 3;
  }

  .crateStatistics {
    flex-direction: row;
    flex-wrap: wrap;
  }

  .mobile-invisible {
    display: none;
  }
}

@media only screen and (min-width: 768px) {
  .crateCard {
    grid-template-rows: auto auto;
    grid-template-columns: 1fr auto;
    grid-template-areas: 
      "title crateStatistics"
      "description crateStatistics";
  }

  .crateDesc {
    -webkit-line-clamp: 3;
  }

  .crateStatistics {
    flex-direction: column;
  }
}

@media only screen and (min-width: 992px) {
  .crateDesc {
    -webkit-line-clamp: 2;
  }
}
</style>
