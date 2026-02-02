<template>
  <v-snackbar
    :model-value="modelValue"
    :color="color"
    :timeout="timeout"
    location="bottom"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    {{ message }}
    <template v-slot:actions>
      <v-btn variant="text" @click="$emit('update:modelValue', false)">Close</v-btn>
    </template>
  </v-snackbar>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: boolean;
  message: string;
  color: string;
  timeout: number;
}>();

defineEmits<{
  'update:modelValue': [value: boolean];
}>();
</script>
