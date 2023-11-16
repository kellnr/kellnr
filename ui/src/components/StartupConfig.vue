<template>
  <h2 class="k-h2">Startup Config</h2>
  <p id="intro">
    Values are set on application startup. See <a href="https://kellnr.io/documentation"
                                                  class="link">Kellnr
    Configuration Documentation</a> for more information.
  </p>
  <startup-config-item toml="data_dir" env="KELLNR_DATA_DIR" :value="settings.registry.data_dir"></startup-config-item>
  <startup-config-item toml="session_age_seconds" env="KELLNR_SESSION_AGE_SECONDS"
                       :value="settings.registry.session_age_seconds"></startup-config-item>
  <startup-config-item toml="api_address" env="KELLNR_API_ADDRESS" :value="settings.origin.hostname"></startup-config-item>
  <startup-config-item toml="api_port" env="KELLNR_API_PORT" :value="settings.api_port"></startup-config-item>
  <startup-config-item toml="api_port_proxy" env="KELLNR_API_PORT_PROXY"
                       :value="settings.origin.port"></startup-config-item>
  <startup-config-item toml="api_protocol" env="KELLNR_API_PROTOCOL"
                       :value="settings.origin.protocol"></startup-config-item>
  <startup-config-item toml="web_address" env="KELLNR_WEB_ADDRESS" :value="settings.web_address"></startup-config-item>
  <startup-config-item toml="crates_io_proxy" env="KELLNR_CRATES_IO_PROXY"
                       :value="settings.proxy.enabled"></startup-config-item>
  <startup-config-item toml="crates_io_num_threads" env="KELLNR_CRATES_IO_NUM_THREADS"
                       :value="settings.crates_io_num_threads"></startup-config-item>
  <startup-config-item toml="log_level" env="KELLNR_LOG_LEVEL" :value="settings.log_level"></startup-config-item>
  <startup-config-item toml="log_level_web_server" env="KELLNR_LOG_LEVEL_WEB_SERVER"
                       :value="settings.log.level_web_server"></startup-config-item>
  <startup-config-item toml="log_format" env="KELLNR_LOG_FORMAT" :value="settings.log.format"></startup-config-item>
  <startup-config-item toml="rustdoc_auto_gen" env="KELLNR_RUSTDOC_AUTO_GEN"
                       :value="settings.docs.enabled"></startup-config-item>
  <startup-config-item toml="cache_size" env="KELLNR_CACHE_SIZE" :value="settings.registry.cache_size_size"></startup-config-item>
  <startup-config-item toml="max_crate_size" env="KELLNR_MAX_CRATE_SIZE"
                       :value="settings.registry.max_crate_size"></startup-config-item>
  <startup-config-item toml="max_docs_size" env="KELLNR_MAX_DOCS_SIZE"
                       :value="settings.docs.max_size"></startup-config-item>
  <startup-config-item toml="enable_git_index" env="KELLNR_GIT_INDEX"
                       :value="settings.git_index"></startup-config-item>
  <startup-config-item toml="enable_git_index" env="KELLNR_AUTH_REQUIRED"
                       :value="settings.registry.auth_required"></startup-config-item>
  <startup-config-item toml="postgresql.enabled" env="KELLNR_POSTGRESQL__ENABLED"
                         :value="settings.postgresql.enabled"></startup-config-item>
  <startup-config-item toml="postgresql.address" env="KELLNR_POSTGRESQL__ADDRESS"
                         :value="settings.postgresql.address"></startup-config-item>
  <startup-config-item toml="postgresql.port" env="KELLNR_POSTGRESQL__PORT"
                         :value="settings.postgresql.port"></startup-config-item>
  <startup-config-item toml="postgresql.db" env="KELLNR_POSTGRESQL__DB"
                         :value="settings.postgresql.db"></startup-config-item>
  <startup-config-item toml="postgresql.user" env="KELLNR_POSTGRESQL__USER"
                         :value="settings.postgresql.user"></startup-config-item>
</template>

<script setup lang="ts">
import {onBeforeMount, ref} from "vue";
import axios from "axios";
import {defaultSettings, Settings} from "@/types/settings";
import StartupConfigItem from "@/components/StartupConfigItem.vue";
import {kellnr_url, SETTINGS, VERSION} from "@/remote-routes";
import {defaultVersionInfo, VersionInfo} from "@/types/version_info";

const settings = ref<Settings>(defaultSettings);

onBeforeMount(() => {
  getStartupConfig();
})

function getStartupConfig() {
  axios.get(SETTINGS)
      .then((res) => {
        settings.value = res.data;
      })
      .catch((err) => {
        console.log(err);
      });
}


function truncate(value: string | undefined, length: number) {
  if (value == undefined) {
    return "";
  }
  if (value.length > length) {
    return value.substring(0, length) + '...';
  } else {
    return value;
  }
}
</script>

<style scoped>
#intro {
  padding-bottom: 1rem;
}
</style>
