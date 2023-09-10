<template>
  <div class="ver clickable glass" @click="openCrateVersionPage">
    <div class="verVersion">
      <div>
        {{ version }}
      </div>
      <div><i class="fas fa-code-branch"></i></div>
    </div>
    <div class="verLastUpdated">
      <div>
        <span class="icon">
          <i class="fas fa-calendar-alt"></i>
        </span>
      </div>
      <div>{{ humanizedLastUpdated }}</div>
    </div>
    <div class="verDownloads">
      <div><i class="fas fa-cloud-download-alt"></i></div>
      <div>{{ downloads }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import dayjs from 'dayjs'
import {computed} from "vue";
import relativeTime from "dayjs/plugin/relativeTime";
import {useRouter} from "vue-router";
dayjs.extend(relativeTime);

const props = defineProps<{
  name: string,
  version: string,
  last_updated: string,
  downloads: string
}>()
const router = useRouter()

const humanizedLastUpdated = computed(() => {
  return dayjs(props.last_updated).fromNow();
})

function openCrateVersionPage() {
  router.push({name: 'Crate', query: {name: props.name, version: props.version}})
}

</script>

<style scoped>
.ver {
  padding: 0.5rem;
  margin: 0 0 0.5rem 0;
  display: grid;
  grid-template-columns: 1fr 2fr 1fr;
  text-align: center;
}

.verVersion {
  padding: 0 0.5rem 0 0;
  border-right-style: solid;
  border-width: 0.1rem;
  font-weight: bold;
}

.verLastUpdated {
  padding: 0 0.5rem 0 0;
  border-right-style: solid;
  border-width: 0.1rem;
}
</style>
