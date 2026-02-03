<template>
  <div>
    <SectionHeader icon="mdi-hammer-wrench" title="Toolchain Management" :count="toolchains.length" />

    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Manage Rust toolchains for distribution via rustup. Upload toolchain archives (.tar.xz) and assign them to release channels.
      </p>

      <ToolchainList
        :toolchains="toolchains"
        :channel-options="channelOptions"
        @channel-change="handleChannelChange"
        @delete-toolchain="handleDeleteToolchain"
        @delete-target="handleDeleteTarget"
      />

      <ToolchainUploadForm
        :channel-options="channelOptions"
        @upload-success="loadToolchains"
      />
    </div>

    <!-- Snackbar for notifications -->
    <NotificationSnackbar
      v-model="notification.snackbar.show"
      :message="notification.snackbar.message"
      :color="notification.snackbar.color"
      :timeout="notification.snackbar.timeout"
    />

    <!-- Delete Confirmation Dialog -->
    <ConfirmDialog
      v-model="dialogOpen"
      :title="dialog.title"
      :message="dialog.message"
      :confirm-color="dialog.confirmColor"
      @confirm="dialog.confirm()"
      @cancel="dialog.cancel()"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref, computed } from "vue"
import { useConfirmCallback, useNotification } from "../composables"
import { toolchainService } from "../services"
import { isSuccess } from "../services/api"
import type { Toolchain } from "../types/toolchain"
import { SectionHeader, ConfirmDialog, NotificationSnackbar } from "./common"
import { ToolchainList, ToolchainUploadForm } from "./toolchain"

const toolchains = ref<Toolchain[]>([])

const { dialog, showConfirm } = useConfirmCallback()
const notification = useNotification()

const dialogOpen = computed({
  get: () => dialog.isOpen.value,
  set: (val: boolean) => { dialog.isOpen.value = val }
})

const channelOptions = ["stable", "beta", "nightly"]

onBeforeMount(() => {
  loadToolchains()
})

async function loadToolchains() {
  const result = await toolchainService.getToolchains()
  if (isSuccess(result)) {
    toolchains.value = result.data
  }
}

async function handleChannelChange(toolchain: Toolchain) {
  if (toolchain.channel) {
    const result = await toolchainService.setChannel(
      toolchain.channel,
      toolchain.name,
      toolchain.version
    )
    if (isSuccess(result)) {
      notification.showSuccess(`Channel "${toolchain.channel}" assigned`)
      await loadToolchains()
    } else {
      notification.showError(result.error.message)
      await loadToolchains()
    }
  }
}

function handleDeleteToolchain(name: string, version: string) {
  showConfirm({
    title: "Delete Toolchain",
    message: `Are you sure you want to delete "${name} ${version}" and ALL its targets? This action cannot be undone.`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await toolchainService.deleteToolchain(name, version)
      if (isSuccess(result)) {
        notification.showSuccess(`Toolchain "${name} ${version}" deleted`)
        await loadToolchains()
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}

function handleDeleteTarget(name: string, version: string, target: string) {
  showConfirm({
    title: "Delete Target",
    message: `Are you sure you want to delete target "${target}" from ${name} ${version}? This action cannot be undone.`,
    confirmColor: "error",
    onConfirm: async () => {
      const result = await toolchainService.deleteToolchainTarget(name, version, target)
      if (isSuccess(result)) {
        notification.showSuccess(`Target "${target}" deleted`)
        await loadToolchains()
      } else {
        notification.showError(result.error.message)
      }
    }
  })
}
</script>

<style scoped>
.section-content {
  padding: 24px;
}

/* Responsive */
@media (max-width: 600px) {
  .section-content {
    padding: 20px;
  }
}
</style>
