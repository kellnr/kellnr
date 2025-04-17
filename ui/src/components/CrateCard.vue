<template>
  <v-card class="crate-card mb-4 rounded-lg transition-swing clickable-card" elevation="2" :color="'primary-lighten-5'"
    variant="elevated" hover @mouseover="isHovered = true" @mouseleave="isHovered = false" @click="navigateToCrate">
    <v-card-text class="pa-4">
      <v-row no-gutters>
        <!-- Origin Logo -->
        <v-col cols="auto" class="mr-4 origin-column">
          <v-avatar :color="'primary-lighten-4'" size="52" class="elevation-1 mr-2" :class="{ 'scale-up': isHovered }">
            <v-img v-if="props.isCache" :src="store.cargoSmallLogo" alt="Crates.io logo" />
            <v-img v-else :src="store.kellnrSmallLogo" alt="Kellnr logo" />
          </v-avatar>
        </v-col>

        <!-- Title and Description Section -->
        <v-col>
          <div class="d-flex flex-wrap align-center mb-2">
            <div class="text-h5 font-weight-bold me-2">
              {{ crate }}
            </div>
            <v-chip size="small" color="primary" variant="flat" class="ml-1 mt-1" :class="{ 'elevated': isHovered }">
              v{{ version }}
            </v-chip>
          </div>
          <div class="text-body-1 text-truncate-3 crate-description">
            {{ desc || "No description available" }}
          </div>
        </v-col>

        <!-- Statistics Section -->
        <v-col cols="12" sm="auto" class="mt-3 mt-sm-0">
          <div class="d-flex flex-wrap gap-4 justify-sm-end">
            <v-tooltip location="top" text="Downloads">
              <template v-slot:activator="{ props: tooltipProps }">
                <div class="d-flex align-center stat-item" v-bind="tooltipProps">
                  <v-icon icon="mdi-cloud-download" size="small" class="mr-2"
                    :color="isHovered ? 'primary' : undefined" />
                  <span class="text-body-2 font-weight-medium">{{ formatNumber(downloads) }}</span>
                </div>
              </template>
            </v-tooltip>

            <v-tooltip location="top" text="Last updated">
              <template v-slot:activator="{ props: tooltipProps }">
                <div class="d-flex align-center stat-item" v-bind="tooltipProps">
                  <v-icon icon="mdi-calendar" size="small" class="mr-2" :color="isHovered ? 'primary' : undefined" />
                  <span class="text-body-2 font-weight-medium">{{ humanizedLastUpdated }}</span>
                </div>
              </template>
            </v-tooltip>

            <div class="d-flex align-center stat-item">
              <v-icon icon="mdi-book-open-variant" size="small" class="mr-2"
                :color="isHovered ? 'primary' : undefined" />
              <a v-if="docLink" v-bind:href="docLink" class="text-decoration-none text-body-2 font-weight-medium"
                target="_blank" @click.stop>
                Documentation
              </a>
              <router-link v-if="!docLink" class="text-decoration-none text-body-2 font-weight-medium" to="/publishdocs"
                @click.stop>
                Documentation
              </router-link>
            </div>
          </div>
        </v-col>
      </v-row>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import utc from "dayjs/plugin/utc";
import { useStore } from "../store/store";
import { useRouter } from "vue-router";

dayjs.extend(relativeTime);
dayjs.extend(utc);
const store = useStore();
const router = useRouter();
const isHovered = ref(false);

const props = defineProps<{
  crate: string
  desc?: string
  downloads: number
  version: string
  updated: string
  docLink?: string
  isCache: boolean
}>()

const humanizedLastUpdated = computed(() => {
  return dayjs.utc(props.updated).fromNow();
})

// Format number with commas for readability
function formatNumber(num: number): string {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// Navigate to crate details or crates.io
function navigateToCrate() {
  if (props.isCache) {
    window.open(`https://crates.io/crates/${props.crate}`, '_blank');
  } else {
    router.push({
      name: 'Crate',
      query: {
        name: props.crate,
        version: props.version
      }
    });
  }
}
</script>

<style scoped>
.origin-column {
  border-right: 1px solid rgba(0, 0, 0, 0.12);
  padding-right: 16px;
}

.text-truncate-3 {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  text-overflow: ellipsis;
}

.gap-4 {
  gap: 16px;
}

.crate-card {
  transition: transform 0.3s ease, box-shadow 0.3s ease;
  overflow: hidden;
  cursor: pointer;
  position: relative;
}

.crate-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 6px 12px rgba(0, 0, 0, 0.1) !important;
}

.crate-card:hover::after {
  opacity: 1;
}

.clickable-card {
  user-select: none;
}

.crate-description {
  line-height: 1.5;
  max-height: 4.5em;
}

.stat-item {
  transition: transform 0.2s ease;
}

.scale-up {
  transform: scale(1.05);
}

.elevated {
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

/* Dark mode adjustments */
:deep(.v-theme--dark) .origin-column {
  border-right: 1px solid rgba(255, 255, 255, 0.12);
}
</style>
