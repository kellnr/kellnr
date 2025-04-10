<template>
  <h2 class="k-h2">Startup Config</h2>
  <p id="intro">
    Values are set on application startup. See
    <a href="https://kellnr.io/documentation" class="link">Kellnr Configuration Documentation</a>
    for more information.
  </p>

  <div class="settingsSection">
    <h3 class="k-h3">Registry</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="registry" toml="data_dir" env="KELLNR_REGISTRY__DATA_DIR"
      :value="settings.registry.data_dir"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="session_age_seconds" env="KELLNR_REGISTRY__SESSION_AGE_SECONDS"
      :value="settings.registry.session_age_seconds"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="cache_size" env="KELLNR_REGISTRY__CACHE_SIZE"
      :value="settings.registry.cache_size"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="max_crate_size" env="KELLNR_REGISTRY__MAX_CRATE_SIZE"
      :value="settings.registry.max_crate_size"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="max_db_connections" env="KELLNR_REGISTRY__MAX_DB_CONNECTIONS"
      :value="settings.registry.max_db_connections"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="auth_required" env="KELLNR_REGISTRY__AUTH_REQUIRED"
      :value="settings.registry.auth_required"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="required_crate_fields" env="KELLNR_REGISTRY__REQUIRED_CRATE_FIELDS"
      :value="settings.registry.required_crate_fields"></startup-config-item>
    <startup-config-item tomlTable="registry" toml="new_crates_restricted" env="KELLNR_REGISTRY__NEW_CRATES_RESTRICTED"
      :value="settings.registry.new_crates_restricted"></startup-config-item>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">Local</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="local" toml="ip" env="KELLNR_LOCAL__IP"
      :value="settings.local.ip"></startup-config-item>
    <startup-config-item tomlTable="local" toml="port" env="KELLNR_LOCAL__PORT"
      :value="settings.local.port"></startup-config-item>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">Origin</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="origin" toml="hostname" env="KELLNR_ORIGIN__HOSTNAME"
      :value="settings.origin.hostname"></startup-config-item>
    <startup-config-item tomlTable="origin" toml="port" env="KELLNR_ORIGIN__PORT"
      :value="settings.origin.port"></startup-config-item>
    <startup-config-item tomlTable="origin" toml="protocol" env="KELLNR_ORIGIN__PROTOCOL"
      :value="settings.origin.protocol"></startup-config-item>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">Log</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="log" toml="level" env="KELLNR_LOG__LEVEL"
      :value="settings.log.level"></startup-config-item>
    <startup-config-item tomlTable="log" toml="format" env="KELLNR_LOG__FORMAT"
      :value="settings.log.format"></startup-config-item>
    <startup-config-item tomlTable="log" toml="level_web_server" env="KELLNR_LOG__LEVEL_WEB_SERVER"
      :value="settings.log.level_web_server"></startup-config-item>
    <div></div>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">Proxy</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="proxy" toml="enabled" env="KELLNR_PROXY__ENABLED"
      :value="settings.proxy.enabled"></startup-config-item>
    <startup-config-item tomlTable="proxy" toml="num_threads" env="KELLNR_PROXY__NUM_THREADS"
      :value="settings.proxy.num_threads"></startup-config-item>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">Docs</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="docs" toml="enabled" env="KELLNR_DOCS__ENABLED"
      :value="settings.docs.enabled"></startup-config-item>
    <startup-config-item tomlTable="docs" toml="max_size" env="KELLNR_DOCS__MAX_SIZE"
      :value="settings.docs.max_size"></startup-config-item>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">PostgreSQL</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="postgresql" toml="enabled" env="KELLNR_POSTGRESQL__ENABLED"
      :value="settings.postgresql.enabled"></startup-config-item>
    <startup-config-item tomlTable="postgresql" toml="address" env="KELLNR_POSTGRESQL__ADDRESS"
      :value="settings.postgresql.address"></startup-config-item>
    <startup-config-item tomlTable="postgresql" toml="port" env="KELLNR_POSTGRESQL__PORT"
      :value="settings.postgresql.port"></startup-config-item>
    <startup-config-item tomlTable="postgresql" toml="db" env="KELLNR_POSTGRESQL__DB"
      :value="settings.postgresql.db"></startup-config-item>
    <startup-config-item tomlTable="postgresql" toml="user" env="KELLNR_POSTGRESQL__USER"
      :value="settings.postgresql.user"></startup-config-item>
  </div>

  <div class="settingsSection">
    <h3 class="k-h3">S3</h3>
    <startup-config-header></startup-config-header>
    <startup-config-item tomlTable="s3" toml="enabled" env="KELLNR_S3__ENABLED"
      :value="settings.s3.enabled"></startup-config-item>
    <startup-config-item tomlTable="s3" toml="access_key" env="KELLNR_S3__ACCESS_KEY"
      :value="settings.s3.access_key"></startup-config-item>
    <startup-config-item tomlTable="s3" toml="secret_key" env="KELLNR_S3__SECRET_KEY"
      :value="settings.s3.secret_key"></startup-config-item>
    <startup-config-item tomlTable="s3" toml="region" env="KELLNR_S3__REGION"
      :value="settings.s3.region"></startup-config-item>
    <startup-config-item tomlTable="s3" toml="endpoint" env="KELLNR_S3__ENDPOINT"
      :value="settings.s3.endpoint"></startup-config-item>
    <startup-config-item tomlTable="s3" toml="allow_http" env="KELLNR_S3__ALLOW_HTTP"
      :value="settings.s3.allow_http"></startup-config-item>

  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from "vue";
import axios from "axios";
import { emptySettings } from "../types/settings";
import type { Settings } from "../types/settings";
import StartupConfigItem from "./StartupConfigItem.vue";
import StartupConfigHeader from "./StartupConfigHeader.vue";
import { SETTINGS } from "../remote-routes";

const settings = ref<Settings>(emptySettings);

onBeforeMount(() => {
  getStartupConfig();
});

function getStartupConfig() {
  axios
    .get(SETTINGS)
    .then((res) => {
      settings.value = res.data;
    })
    .catch((err) => {
      console.log(err);
    });
}
</script>

<style scoped>
#intro {
  padding-bottom: 1rem;
}
</style>

<style scoped>
.settingsSection {
  margin-bottom: 1rem;
}

.k-h3 {
  margin-bottom: 0rem;
  margin-top: 2rem;
}
</style>
