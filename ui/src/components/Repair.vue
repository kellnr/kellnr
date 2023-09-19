<template>
  <h2 class="k-h2">Repair crates.io Index</h2>
  <p class="k-p">
    Attention: Only applies to the <i>git index</i> which is deprecated and not enabled by default.
    <br />
    <br />
    If the <i>crates.io</i> index, used by the cache, is out of sync and cannot repair itself, this button triggers a
    re-creation of the index. This can take several minutes, in which no <i>crates</i> from the <i>crates.io</i> proxy can
    be served.
    A restart of Kellnr is not required.
  </p>
  <div class="control">
    <button class="button is-info" @click.prevent="deleteCratesIoIndex()">Reset</button>
  </div>
  <br />
  <status-notification :status="deleteTriggeredStatus" @update:clear="deleteTriggeredStatus = $event">
    {{ deleteTriggeredMsg }}
  </status-notification>
</template>

<script setup lang="ts">
import StatusNotification from "../components/StatusNotification.vue";
import { ref } from "vue";
import axios from "axios";
import { CRATES_IO_INDEX, kellnr_url } from "@/remote-routes";

const deleteTriggeredStatus = ref("")
const deleteTriggeredMsg = ref("")

function deleteCratesIoIndex() {
  axios
    .delete(CRATES_IO_INDEX)
    .then((_res) => {
      deleteTriggeredMsg.value = "Triggered crates.io index re-creation.";
      deleteTriggeredStatus.value = "Success";
    })
    .catch((error) => {
      deleteTriggeredMsg.value = "Failed to trigger crates.io index re-creation.";
      deleteTriggeredStatus.value = "Error"
      console.log(error)
    })
}
</script>

<style scoped></style>
