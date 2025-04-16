<template>
  <v-card class="mb-2" elevation="1" :elevation-on-hover="3" @click="openCrateVersionPage">
    <v-row no-gutters align="center">
      <v-col cols="12" sm="4" md="3" lg="2" class="pa-3">
        <v-card-text class="pa-0 d-flex justify-center align-center">
          <span class="font-weight-bold text-body-1 me-2">{{ version }}</span>
          <v-icon size="small" color="info">mdi-tag-outline</v-icon>
        </v-card-text>
      </v-col>

      <v-divider vertical></v-divider>

      <v-col cols="12" sm="4" md="6" lg="7" class="pa-3">
        <v-card-text class="pa-0 d-flex justify-center align-center">
          <v-icon size="small" class="me-2" color="info">mdi-calendar</v-icon>
          <span>{{ humanizedLastUpdated }}</span>
        </v-card-text>
      </v-col>

      <v-divider vertical></v-divider>

      <v-col cols="12" sm="4" md="3" lg="3" class="pa-3">
        <v-card-text class="pa-0 d-flex justify-center align-center">
          <v-icon size="small" class="me-2" color="success">mdi-download</v-icon>
          <span>{{ downloads }}</span>
        </v-card-text>
      </v-col>
    </v-row>
    <v-tooltip activator="parent" location="bottom">
      View details for version {{ version }}
    </v-tooltip>
  </v-card>
</template>

<script setup lang="ts">
import dayjs from 'dayjs'
import { computed } from "vue";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import { useRouter } from "vue-router";

dayjs.extend(relativeTime);
dayjs.extend(utc);

const props = defineProps<{
  name: string,
  version: string,
  last_updated: string,
  downloads: string
}>();

const router = useRouter();

const humanizedLastUpdated = computed(() => {
  return dayjs.utc(props.last_updated).fromNow();
});

function openCrateVersionPage() {
  router.push({ name: 'Crate', query: { name: props.name, version: props.version } });
}
</script>
