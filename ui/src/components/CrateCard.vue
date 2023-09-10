<template>
  <div class="crateCard glass">
<!--    <div class="firstColumn">-->
<!--      <span class="boxIcon"><i class="fas fa-box"></i></span>-->
<!--    </div>-->
    <div class="secondColumn">
      <div>
        <router-link class="crateName" :to="{name: 'Crate', query: {name: crate, version: version}}">{{crate}}</router-link>
        <span class="crateVersion">v{{ version }}</span>
      </div>
      <div class="secondRow">
        <span class="docs" v-if="docLink">
          <a v-bind:href="docLink" class="clickable" target="_blank">Documentation</a>
        </span>
          <span class="docs" v-else>
          <router-link class="clickable" to="/publishdocs">Add Documentation</router-link>
        </span>
      </div>
    </div>
    <div class="thirdColumn">
      <span v-if="desc" class="crateDesc">
        {{ desc }}
      </span>
      <span v-else>
        <i>no description available</i>
      </span>
    </div>
    <div class="fourthColumn">
      <div class="crateIconInfo">
        <span class="crateIcon"><i class="fas fa-cloud-download-alt"></i></span>
        <span>Downloads: {{ downloads }}</span>
      </div>
      <div class="crateIconInfo">
        <span class="crateIcon"><i class="fas fa-calendar-alt"></i></span>
        <span>Updated: {{ humanizedLastUpdated }}</span>
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
  grid-template-columns: auto 2fr 3fr 1fr;
  text-align: left;
  margin-bottom: 1rem;
  padding-bottom: 0.5rem;
}

.secondColumn {
  grid-column: 2;
  display: grid;
  grid-template-rows: auto auto;
}

.thirdColumn {
  grid-column: 3;
  grid-row: 1;
  display: grid;
}

.fourthColumn {
  grid-column: 4;
  display: grid;
  grid-template-rows: auto auto;
}

.secondRow {
  grid-row: 2;
  padding-top: 0.5rem;
  font-size: smaller;
}

.crateName {
  font-weight: bolder;
  font-size: larger;
  padding-right: 0.5rem;
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
</style>
