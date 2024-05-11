<template>
  <div class="field has-addons">
    <div class="control">
      <input
        v-model="props.content"
        ref="input"
        class="input is-info k-copy-input"
        type="text"
        disabled
      />
    </div>
    <div class="control k-hint-container">
      <button @click.prevent="copyToClipboard" class="button is-info">
        <span v-if="copyIcon === 'copy'" class="icon">
          <i class="fa-regular fa-copy"></i>
        </span>
        <span v-else-if="copyIcon === 'success'" class="icon">
          <i class="fa-solid fa-check"></i>
        </span>
        <span v-else-if="copyIcon === 'failed'" class="icon">
          <i class="fa-solid fa-x"></i>
        </span>
      </button>
      <p v-if="copyIcon === 'success'" class="help is-success k-hint">
        Copied!
      </p>
      <p v-else-if="copyIcon === 'failed'" class="help is-danger k-hint">
        Copy failed!
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";

const props = defineProps({ content: { type: String, required: true } });

type CopyIcon = "copy" | "success" | "failed";
const copyIcon = ref<CopyIcon>("copy");

function copyToClipboard() {
  const updateIconCb = (copyIconUpdate: CopyIcon, resetTimeout = 2500) => {
    copyIcon.value = copyIconUpdate;
    setTimeout(() => {
      copyIcon.value = "copy";
    }, resetTimeout);
  };

  navigator.clipboard
    .writeText(props.content)
    .then(() => {
      updateIconCb("success");
    })
    .catch((err) => {
      updateIconCb("failed");
      console.error("Failed to copy content to clipboard", err);
    });
}
</script>

<style>
.k-hint-container {
  position: relative;
}

.k-hint {
  position: absolute;
  top: 100%;
  left: 50%;
  transform: translateX(-50%);
  z-index: 1050;
}

.k-copy-input {
  /* Override the default "forbidden" cursor */
  cursor: default !important;
}
</style>
