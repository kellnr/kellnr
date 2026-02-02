<template>
  <v-snackbar
    v-model="snackbar.show"
    :color="snackbar.color"
    :timeout="snackbar.timeout"
    location="bottom"
  >
    {{ snackbar.message }}
    <template v-slot:actions>
      <v-btn variant="text" @click="close">Close</v-btn>
    </template>
  </v-snackbar>
</template>

<script setup lang="ts">
defineProps<{
  snackbar: {
    show: boolean;
    message: string;
    color: string;
    timeout: number;
  };
}>();

const emit = defineEmits<{
  'close': [];
}>();

function close() {
  emit('close');
}
</script>
