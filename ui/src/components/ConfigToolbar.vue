<template>
  <div class="config-toolbar">
    <v-btn
      size="small"
      :variant="filterActive ? 'flat' : 'tonal'"
      :color="filterActive ? 'primary' : 'default'"
      @click="$emit('update:filterActive', !filterActive)"
    >
      <v-icon start size="small">{{ filterActive ? 'mdi-filter' : 'mdi-filter-outline' }}</v-icon>
      Configured only
    </v-btn>
    <div class="toolbar-spacer"></div>
    <v-btn size="small" variant="tonal" @click="$emit('expandAll')">
      <v-icon start size="small">mdi-unfold-more-horizontal</v-icon>
      Expand all
    </v-btn>
    <v-btn size="small" variant="tonal" @click="$emit('collapseAll')">
      <v-icon start size="small">mdi-unfold-less-horizontal</v-icon>
      Collapse all
    </v-btn>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  filterActive: boolean;
}>();

defineEmits<{
  'update:filterActive': [value: boolean];
  'expandAll': [];
  'collapseAll': [];
}>();
</script>

<style scoped>
.config-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 16px;
}

.toolbar-spacer {
  flex: 1;
}
</style>
