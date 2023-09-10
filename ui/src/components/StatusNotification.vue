<template>
  <div v-if="status !== ''" class="notification is-light" :class="[status === 'Error' ? 'is-danger' : 'is-success']">
    <button class="delete" @click.prevent="clearStatus()"></button>
    <slot></slot>
  </div>
</template>

<script setup lang="ts">
import {defineProps} from "vue";

const props = defineProps<{
  status: string
}>()

const emit = defineEmits(['update:clear'])

function clearStatus() {
  emit("update:clear", "");
}
</script>

<style>
</style>
