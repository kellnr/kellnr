<template>
  <v-dialog v-model="modelValue" max-width="450">
    <v-card class="confirm-dialog">
      <div class="dialog-header" :class="headerColorClass">
        <v-icon :icon="icon" :color="iconColor" size="small" class="me-3"></v-icon>
        <span class="text-h6 font-weight-bold">{{ title }}</span>
      </div>

      <v-card-text class="pa-5">
        <p class="text-body-1 mb-2">{{ message }}</p>
        <p v-if="subMessage" class="text-body-2 text-medium-emphasis mb-0">
          {{ subMessage }}
        </p>
      </v-card-text>

      <v-card-actions class="pa-4 pt-0">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="handleCancel">
          {{ cancelText }}
        </v-btn>
        <v-btn :color="confirmColor" variant="flat" @click="handleConfirm">
          <v-icon v-if="confirmIcon" :icon="confirmIcon" size="small" class="me-1"></v-icon>
          {{ confirmText }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(defineProps<{
  modelValue: boolean;
  title: string;
  message: string;
  subMessage?: string;
  confirmText?: string;
  cancelText?: string;
  confirmColor?: string;
  confirmIcon?: string;
  icon?: string;
}>(), {
  confirmText: 'Confirm',
  cancelText: 'Cancel',
  confirmColor: 'primary',
  icon: 'mdi-alert-circle',
});

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
  'confirm': [];
  'cancel': [];
}>();

const modelValue = computed({
  get: () => props.modelValue,
  set: (val: boolean) => emit('update:modelValue', val)
});

const iconColor = computed(() => {
  if (props.confirmColor === 'error') return 'error';
  if (props.confirmColor === 'warning') return 'warning';
  return 'warning';
});

const headerColorClass = computed(() => {
  if (props.confirmColor === 'error') return 'header-error';
  return 'header-warning';
});

function handleConfirm() {
  emit('confirm');
  emit('update:modelValue', false);
}

function handleCancel() {
  emit('cancel');
  emit('update:modelValue', false);
}
</script>

<style scoped>
.confirm-dialog {
  border-radius: 12px;
  overflow: hidden;
}

.dialog-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
}

.dialog-header.header-warning {
  background: rgba(var(--v-theme-warning), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-warning), 0.2);
}

.dialog-header.header-error {
  background: rgba(var(--v-theme-error), 0.08);
  border-bottom: 1px solid rgba(var(--v-theme-error), 0.2);
}
</style>
