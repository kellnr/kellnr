<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-tune" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">Startup Configuration</span>
      <span class="section-count">8 sections</span>
    </div>

    <!-- Content -->
    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-5">
        Configuration values are set on application startup and cannot be changed at runtime.
        See the <a href="https://kellnr.io/documentation" class="text-primary font-weight-medium">Kellnr Documentation</a> for details.
      </p>

      <!-- Config Sections -->
      <v-expansion-panels variant="accordion" class="config-panels">
        <!-- Registry Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-package-variant-closed" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Registry</span>
              <v-chip size="x-small" class="ms-2" color="primary" variant="tonal">14 settings</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Data Directory</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.data_dir) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.data_dir</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__DATA_DIR</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Session Age (seconds)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.session_age_seconds) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.session_age_seconds</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__SESSION_AGE_SECONDS</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Cache Size</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.cache_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.cache_size</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__CACHE_SIZE</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Max Crate Size</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.max_crate_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.max_crate_size</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__MAX_CRATE_SIZE</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Max DB Connections</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.max_db_connections) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.max_db_connections</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__MAX_DB_CONNECTIONS</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Auth Required</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.auth_required ? 'enabled' : 'disabled']">{{ settings.registry.auth_required ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.auth_required</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__AUTH_REQUIRED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Required Crate Fields</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.required_crate_fields) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.required_crate_fields</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__REQUIRED_CRATE_FIELDS</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">New Crates Restricted</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.new_crates_restricted ? 'enabled' : 'disabled']">{{ settings.registry.new_crates_restricted ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.new_crates_restricted</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__NEW_CRATES_RESTRICTED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Allow Ownerless Crates</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.allow_ownerless_crates ? 'enabled' : 'disabled']">{{ settings.registry.allow_ownerless_crates ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.allow_ownerless_crates</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__ALLOW_OWNERLESS_CRATES</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Token Cache Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.token_cache_enabled ? 'enabled' : 'disabled']">{{ settings.registry.token_cache_enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_cache_enabled</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_CACHE_ENABLED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Token Cache TTL (seconds)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_cache_ttl_seconds) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_cache_ttl_seconds</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_CACHE_TTL_SECONDS</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Token Cache Max Capacity</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_cache_max_capacity) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_cache_max_capacity</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_CACHE_MAX_CAPACITY</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Token DB Retry Count</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_db_retry_count) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_db_retry_count</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_DB_RETRY_COUNT</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Token DB Retry Delay (ms)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_db_retry_delay_ms) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_db_retry_delay_ms</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_DB_RETRY_DELAY_MS</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Local Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-server" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Local</span>
              <v-chip size="x-small" class="ms-2" color="primary" variant="tonal">2 settings</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">IP</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.local.ip) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">local.ip</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOCAL__IP</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Port</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.local.port) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">local.port</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOCAL__PORT</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Origin Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-earth" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Origin</span>
              <v-chip size="x-small" class="ms-2" color="primary" variant="tonal">4 settings</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Hostname</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.hostname) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.hostname</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__HOSTNAME</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Port</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.port) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.port</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__PORT</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Protocol</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.protocol) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.protocol</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__PROTOCOL</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Path</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.path) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.path</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__PATH</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Log Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-file-document-outline" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Log</span>
              <v-chip size="x-small" class="ms-2" color="primary" variant="tonal">3 settings</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Level</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.log.level) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">log.level</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOG__LEVEL</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Format</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.log.format) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">log.format</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOG__FORMAT</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Level Web Server</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.log.level_web_server) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">log.level_web_server</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOG__LEVEL_WEB_SERVER</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Proxy Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-transit-connection-variant" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Proxy</span>
              <v-chip size="x-small" class="ms-2" :color="settings.proxy.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.proxy.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.proxy.enabled ? 'enabled' : 'disabled']">{{ settings.proxy.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.enabled</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__ENABLED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Number of Threads</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.proxy.num_threads) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.num_threads</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__NUM_THREADS</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Download on Update</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.proxy.download_on_update ? 'enabled' : 'disabled']">{{ settings.proxy.download_on_update ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.download_on_update</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__DOWNLOAD_ON_UPDATE</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">URL</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.proxy.url) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.url</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__URL</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Index URL</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.proxy.index) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.index</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__INDEX</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Docs Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-file-document-multiple-outline" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Docs</span>
              <v-chip size="x-small" class="ms-2" :color="settings.docs.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.docs.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.docs.enabled ? 'enabled' : 'disabled']">{{ settings.docs.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">docs.enabled</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_DOCS__ENABLED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Max Size</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.docs.max_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">docs.max_size</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_DOCS__MAX_SIZE</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- PostgreSQL Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-database" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">PostgreSQL</span>
              <v-chip size="x-small" class="ms-2" :color="settings.postgresql.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.postgresql.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.postgresql.enabled ? 'enabled' : 'disabled']">{{ settings.postgresql.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.enabled</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__ENABLED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Address</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.address) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.address</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__ADDRESS</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Port</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.port) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.port</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__PORT</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Database</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.db) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.db</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__DB</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">User</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.user) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.user</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__USER</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- S3 Section -->
        <v-expansion-panel>
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-cloud-outline" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">S3 Storage</span>
              <v-chip size="x-small" class="ms-2" :color="settings.s3.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.s3.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.s3.enabled ? 'enabled' : 'disabled']">{{ settings.s3.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.enabled</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ENABLED</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Access Key</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.access_key) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.access_key</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ACCESS_KEY</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Secret Key</div>
                <div class="config-value-cell"><span class="value-text secret">{{ settings.s3.secret_key ? '********' : '-' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.secret_key</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__SECRET_KEY</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Region</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.region) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.region</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__REGION</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Endpoint</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.endpoint) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.endpoint</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ENDPOINT</code></div>
                </div>
              </div>
              <div class="config-row">
                <div class="config-label">Allow HTTP</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.s3.allow_http ? 'warning' : 'disabled']">{{ settings.s3.allow_http ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.allow_http</code></div>
                  <div class="config-ref"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ALLOW_HTTP</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onBeforeMount, ref } from "vue";
import axios from "axios";
import { emptySettings } from "../types/settings";
import type { Settings } from "../types/settings";
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

function formatValue(value: any): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return value.length > 0 ? value.join(', ') : '-';
  return value.toString() || '-';
}
</script>

<style scoped>
.section-header {
  display: flex;
  align-items: center;
  padding: 16px 24px;
  background: rgba(var(--v-theme-primary), 0.05);
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.section-count {
  margin-left: auto;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 12px;
}

.section-content {
  padding: 24px;
}

/* Panel Styling */
.config-panels {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.config-panels :deep(.v-expansion-panel) {
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-outline));
  border-radius: 8px !important;
  margin-bottom: 0;
}

.config-panels :deep(.v-expansion-panel::before) {
  box-shadow: none;
}

.config-panels :deep(.v-expansion-panel--active) {
  border-color: rgb(var(--v-theme-primary));
}

.panel-title {
  min-height: 56px;
}

.panel-header {
  display: flex;
  align-items: center;
}

.panel-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: rgba(var(--v-theme-primary), 0.1);
  color: rgb(var(--v-theme-primary));
  margin-right: 12px;
}

/* Config List */
.config-list {
  display: flex;
  flex-direction: column;
}

.config-row {
  display: grid;
  grid-template-columns: 200px 160px 1fr;
  gap: 16px;
  align-items: start;
  padding: 12px 0;
  border-bottom: 1px solid rgb(var(--v-theme-outline));
}

.config-row:last-child {
  border-bottom: none;
}

.config-label {
  font-weight: 500;
  font-size: 14px;
  color: rgb(var(--v-theme-on-surface));
}

.config-value-cell {
  display: flex;
  align-items: center;
}

.value-text {
  font-family: 'Roboto Mono', monospace;
  font-size: 13px;
  background: rgba(var(--v-theme-primary), 0.08);
  color: rgb(var(--v-theme-on-surface));
  padding: 4px 8px;
  border-radius: 4px;
  word-break: break-all;
}

.value-text.secret {
  letter-spacing: 2px;
}

.boolean-badge {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: 600;
}

.boolean-badge.enabled {
  background: rgba(var(--v-theme-success), 0.15);
  color: rgb(var(--v-theme-success));
}

.boolean-badge.disabled {
  background: rgba(var(--v-theme-on-surface), 0.08);
  color: rgb(var(--v-theme-on-surface-variant));
}

.boolean-badge.warning {
  background: rgba(var(--v-theme-warning), 0.15);
  color: rgb(var(--v-theme-warning));
}

.config-refs {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.config-ref {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ref-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 42px;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.ref-badge.toml {
  background: rgba(103, 58, 183, 0.15);
  color: #7c4dff;
}

.ref-badge.env {
  background: rgba(0, 150, 136, 0.15);
  color: #00bfa5;
}

.ref-value {
  font-family: 'Roboto Mono', monospace;
  font-size: 12px;
  background: rgba(var(--v-theme-on-surface), 0.06);
  padding: 2px 6px;
  border-radius: 4px;
  color: rgb(var(--v-theme-on-surface-variant));
}

/* Responsive */
@media (max-width: 960px) {
  .config-row {
    grid-template-columns: 1fr;
    gap: 8px;
  }

  .config-label {
    font-weight: 600;
  }

  .config-refs {
    flex-direction: row;
    flex-wrap: wrap;
  }
}

@media (max-width: 600px) {
  .section-header {
    padding: 16px 20px;
  }

  .section-content {
    padding: 20px;
  }
}
</style>
