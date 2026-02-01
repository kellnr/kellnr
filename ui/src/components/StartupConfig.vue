<template>
  <div>
    <!-- Header -->
    <div class="section-header">
      <v-icon icon="mdi-tune" size="small" color="primary" class="me-3"></v-icon>
      <span class="text-h6 font-weight-bold">Startup Configuration</span>
      <span v-if="totalConfigured > 0" class="section-count configured">{{ totalConfigured }}/{{ totalSettings }} configured</span>
      <span class="section-count">{{ visibleSectionsCount }} sections</span>
    </div>

    <!-- Content -->
    <div class="section-content">
      <p class="text-body-2 text-medium-emphasis mb-4">
        Configuration values are set on application startup and cannot be changed at runtime.
        See the <a href="https://kellnr.io/documentation" class="text-primary font-weight-medium">Kellnr Documentation</a> for details.
      </p>

      <!-- Filter toggle -->
      <div class="filter-toggle mb-4">
        <v-switch
          v-model="showOnlyConfigured"
          label="Show only configured values"
          density="compact"
          hide-details
          color="primary"
        ></v-switch>
      </div>

      <!-- Config Sections -->
      <v-expansion-panels variant="accordion" class="config-panels">
        <!-- Registry Section -->
        <v-expansion-panel v-show="isSectionVisible('registry')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-package-variant-closed" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Registry</span>
              <v-chip size="x-small" class="ms-2" :color="configuredCounts.registry > 0 ? 'success' : 'primary'" variant="tonal">{{ badgeText('registry') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.data_dir')">
                <div class="config-label">Data Directory</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.data_dir) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.data_dir', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.data_dir</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.data_dir', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__DATA_DIR</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.data_dir', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-data-dir, -d</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.session_age_seconds')">
                <div class="config-label">Session Age (seconds)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.session_age_seconds) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.session_age_seconds', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.session_age_seconds</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.session_age_seconds', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__SESSION_AGE_SECONDS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.session_age_seconds', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-session-age</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.cache_size')">
                <div class="config-label">Cache Size</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.cache_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.cache_size', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.cache_size</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.cache_size', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__CACHE_SIZE</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.cache_size', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-cache-size</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.max_crate_size')">
                <div class="config-label">Max Crate Size</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.max_crate_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.max_crate_size', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.max_crate_size</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.max_crate_size', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__MAX_CRATE_SIZE</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.max_crate_size', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-max-crate-size</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.max_db_connections')">
                <div class="config-label">Max DB Connections</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.max_db_connections) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.max_db_connections', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.max_db_connections</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.max_db_connections', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__MAX_DB_CONNECTIONS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.max_db_connections', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-max-db-connections</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.auth_required')">
                <div class="config-label">Auth Required</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.auth_required ? 'enabled' : 'disabled']">{{ settings.registry.auth_required ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.auth_required', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.auth_required</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.auth_required', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__AUTH_REQUIRED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.auth_required', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-auth-required</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.required_crate_fields')">
                <div class="config-label">Required Crate Fields</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.required_crate_fields) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.required_crate_fields', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.required_crate_fields</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.required_crate_fields', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__REQUIRED_CRATE_FIELDS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.required_crate_fields', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-required-crate-fields</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.new_crates_restricted')">
                <div class="config-label">New Crates Restricted</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.new_crates_restricted ? 'enabled' : 'disabled']">{{ settings.registry.new_crates_restricted ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.new_crates_restricted', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.new_crates_restricted</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.new_crates_restricted', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__NEW_CRATES_RESTRICTED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.new_crates_restricted', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-new-crates-restricted</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.cookie_signing_key')">
                <div class="config-label">Cookie Signing Key</div>
                <div class="config-value-cell"><span class="value-text secret">{{ settings.registry.cookie_signing_key ? '********' : '-' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.cookie_signing_key', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.cookie_signing_key</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.cookie_signing_key', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__COOKIE_SIGNING_KEY</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.cookie_signing_key', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-cookie-signing-key</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.allow_ownerless_crates')">
                <div class="config-label">Allow Ownerless Crates</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.allow_ownerless_crates ? 'enabled' : 'disabled']">{{ settings.registry.allow_ownerless_crates ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.allow_ownerless_crates', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.allow_ownerless_crates</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.allow_ownerless_crates', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__ALLOW_OWNERLESS_CRATES</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.allow_ownerless_crates', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-allow-ownerless-crates</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.token_cache_enabled')">
                <div class="config-label">Token Cache Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.registry.token_cache_enabled ? 'enabled' : 'disabled']">{{ settings.registry.token_cache_enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_enabled', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_cache_enabled</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_enabled', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_CACHE_ENABLED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_enabled', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-token-cache-enabled</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.token_cache_ttl_seconds')">
                <div class="config-label">Token Cache TTL (seconds)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_cache_ttl_seconds) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_ttl_seconds', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_cache_ttl_seconds</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_ttl_seconds', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_CACHE_TTL_SECONDS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_ttl_seconds', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-token-cache-ttl</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.token_cache_max_capacity')">
                <div class="config-label">Token Cache Max Capacity</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_cache_max_capacity) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_max_capacity', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_cache_max_capacity</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_max_capacity', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_CACHE_MAX_CAPACITY</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_cache_max_capacity', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-token-cache-max-capacity</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.token_db_retry_count')">
                <div class="config-label">Token DB Retry Count</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_db_retry_count) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_db_retry_count', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_db_retry_count</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_db_retry_count', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_DB_RETRY_COUNT</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_db_retry_count', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-token-db-retry-count</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('registry.token_db_retry_delay_ms')">
                <div class="config-label">Token DB Retry Delay (ms)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.registry.token_db_retry_delay_ms) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_db_retry_delay_ms', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">registry.token_db_retry_delay_ms</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_db_retry_delay_ms', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_REGISTRY__TOKEN_DB_RETRY_DELAY_MS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('registry.token_db_retry_delay_ms', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--registry-token-db-retry-delay</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Local Section -->
        <v-expansion-panel v-show="isSectionVisible('local')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-server" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Local</span>
              <v-chip size="x-small" class="ms-2" :color="configuredCounts.local > 0 ? 'success' : 'primary'" variant="tonal">{{ badgeText('local') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('local.ip')">
                <div class="config-label">IP</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.local.ip) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('local.ip', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">local.ip</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('local.ip', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOCAL__IP</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('local.ip', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--local-ip</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('local.port')">
                <div class="config-label">Port</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.local.port) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('local.port', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">local.port</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('local.port', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOCAL__PORT</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('local.port', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--local-port, -p</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Origin Section -->
        <v-expansion-panel v-show="isSectionVisible('origin')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-earth" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Origin</span>
              <v-chip size="x-small" class="ms-2" :color="configuredCounts.origin > 0 ? 'success' : 'primary'" variant="tonal">{{ badgeText('origin') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('origin.hostname')">
                <div class="config-label">Hostname</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.hostname) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.hostname', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.hostname</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.hostname', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__HOSTNAME</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.hostname', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--origin-hostname</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('origin.port')">
                <div class="config-label">Port</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.port) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.port', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.port</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.port', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__PORT</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.port', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--origin-port</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('origin.protocol')">
                <div class="config-label">Protocol</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.protocol) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.protocol', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.protocol</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.protocol', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__PROTOCOL</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('origin.path')">
                <div class="config-label">Path</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.origin.path) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.path', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">origin.path</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.path', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_ORIGIN__PATH</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('origin.path', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--origin-path</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Log Section -->
        <v-expansion-panel v-show="isSectionVisible('log')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-file-document-outline" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Log</span>
              <v-chip size="x-small" class="ms-2" :color="configuredCounts.log > 0 ? 'success' : 'primary'" variant="tonal">{{ badgeText('log') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('log.level')">
                <div class="config-label">Level</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.log.level) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.level', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">log.level</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.level', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOG__LEVEL</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.level', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--log-level, -l</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('log.format')">
                <div class="config-label">Format</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.log.format) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.format', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">log.format</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.format', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOG__FORMAT</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.format', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--log-format</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('log.level_web_server')">
                <div class="config-label">Level Web Server</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.log.level_web_server) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.level_web_server', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">log.level_web_server</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.level_web_server', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_LOG__LEVEL_WEB_SERVER</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('log.level_web_server', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--log-level-web-server</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Proxy Section -->
        <v-expansion-panel v-show="isSectionVisible('proxy')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-transit-connection-variant" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Proxy</span>
              <v-chip size="x-small" class="ms-2" :color="settings.proxy.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.proxy.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
              <v-chip v-if="configuredCounts.proxy > 0" size="x-small" class="ms-2" color="success" variant="tonal">{{ badgeText('proxy') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('proxy.enabled')">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.proxy.enabled ? 'enabled' : 'disabled']">{{ settings.proxy.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.enabled', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.enabled</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.enabled', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__ENABLED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.enabled', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--proxy-enabled</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('proxy.num_threads')">
                <div class="config-label">Number of Threads</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.proxy.num_threads) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.num_threads', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.num_threads</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.num_threads', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__NUM_THREADS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.num_threads', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--proxy-num-threads</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('proxy.download_on_update')">
                <div class="config-label">Download on Update</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.proxy.download_on_update ? 'enabled' : 'disabled']">{{ settings.proxy.download_on_update ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.download_on_update', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.download_on_update</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.download_on_update', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__DOWNLOAD_ON_UPDATE</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.download_on_update', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--proxy-download-on-update</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('proxy.url')">
                <div class="config-label">URL</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.proxy.url) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.url', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.url</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.url', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__URL</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.url', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--proxy-url</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('proxy.index')">
                <div class="config-label">Index URL</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.proxy.index) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.index', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">proxy.index</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.index', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_PROXY__INDEX</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('proxy.index', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--proxy-index</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Docs Section -->
        <v-expansion-panel v-show="isSectionVisible('docs')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-file-document-multiple-outline" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Docs</span>
              <v-chip size="x-small" class="ms-2" :color="settings.docs.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.docs.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
              <v-chip v-if="configuredCounts.docs > 0" size="x-small" class="ms-2" color="success" variant="tonal">{{ badgeText('docs') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('docs.enabled')">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.docs.enabled ? 'enabled' : 'disabled']">{{ settings.docs.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('docs.enabled', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">docs.enabled</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('docs.enabled', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_DOCS__ENABLED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('docs.enabled', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--docs-enabled</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('docs.max_size')">
                <div class="config-label">Max Size</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.docs.max_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('docs.max_size', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">docs.max_size</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('docs.max_size', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_DOCS__MAX_SIZE</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('docs.max_size', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--docs-max-size</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- PostgreSQL Section -->
        <v-expansion-panel v-show="isSectionVisible('postgresql')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-database" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">PostgreSQL</span>
              <v-chip size="x-small" class="ms-2" :color="settings.postgresql.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.postgresql.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
              <v-chip v-if="configuredCounts.postgresql > 0" size="x-small" class="ms-2" color="success" variant="tonal">{{ badgeText('postgresql') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('postgresql.enabled')">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.postgresql.enabled ? 'enabled' : 'disabled']">{{ settings.postgresql.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.enabled', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.enabled</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.enabled', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__ENABLED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.enabled', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--postgresql-enabled</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('postgresql.address')">
                <div class="config-label">Address</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.address) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.address', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.address</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.address', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__ADDRESS</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.address', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--postgresql-address</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('postgresql.port')">
                <div class="config-label">Port</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.port) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.port', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.port</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.port', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__PORT</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.port', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--postgresql-port</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('postgresql.db')">
                <div class="config-label">Database</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.db) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.db', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.db</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.db', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__DB</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.db', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--postgresql-db</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('postgresql.user')">
                <div class="config-label">User</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.postgresql.user) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.user', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">postgresql.user</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.user', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_POSTGRESQL__USER</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('postgresql.user', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--postgresql-user</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- S3 Section -->
        <v-expansion-panel v-show="isSectionVisible('s3')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-cloud-outline" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">S3 Storage</span>
              <v-chip size="x-small" class="ms-2" :color="settings.s3.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.s3.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
              <v-chip v-if="configuredCounts.s3 > 0" size="x-small" class="ms-2" color="success" variant="tonal">{{ badgeText('s3') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.enabled')">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.s3.enabled ? 'enabled' : 'disabled']">{{ settings.s3.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.enabled', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.enabled</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.enabled', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ENABLED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.enabled', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-enabled</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.access_key')">
                <div class="config-label">Access Key</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.access_key) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.access_key', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.access_key</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.access_key', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ACCESS_KEY</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.access_key', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-access-key</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.secret_key')">
                <div class="config-label">Secret Key</div>
                <div class="config-value-cell"><span class="value-text secret">{{ settings.s3.secret_key ? '********' : '-' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.secret_key', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.secret_key</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.secret_key', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__SECRET_KEY</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.secret_key', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-secret-key</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.region')">
                <div class="config-label">Region</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.region) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.region', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.region</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.region', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__REGION</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.region', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-region</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.endpoint')">
                <div class="config-label">Endpoint</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.endpoint) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.endpoint', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.endpoint</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.endpoint', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ENDPOINT</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.endpoint', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-endpoint</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.allow_http')">
                <div class="config-label">Allow HTTP</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.s3.allow_http ? 'warning' : 'disabled']">{{ settings.s3.allow_http ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.allow_http', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.allow_http</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.allow_http', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__ALLOW_HTTP</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.allow_http', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-allow-http</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.crates_bucket')">
                <div class="config-label">Crates Bucket</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.crates_bucket) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.crates_bucket', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.crates_bucket</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.crates_bucket', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__CRATES_BUCKET</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.crates_bucket', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-crates-bucket</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.cratesio_bucket')">
                <div class="config-label">Crates.io Bucket</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.cratesio_bucket) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.cratesio_bucket', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.cratesio_bucket</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.cratesio_bucket', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__CRATESIO_BUCKET</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.cratesio_bucket', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-cratesio-bucket</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('s3.toolchain_bucket')">
                <div class="config-label">Toolchain Bucket</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.s3.toolchain_bucket) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.toolchain_bucket', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">s3.toolchain_bucket</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.toolchain_bucket', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_S3__TOOLCHAIN_BUCKET</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('s3.toolchain_bucket', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--s3-toolchain-bucket</code></div>
                </div>
              </div>
            </div>
          </v-expansion-panel-text>
        </v-expansion-panel>

        <!-- Toolchain Section -->
        <v-expansion-panel v-show="isSectionVisible('toolchain')">
          <v-expansion-panel-title class="panel-title">
            <div class="panel-header">
              <div class="panel-icon-wrapper">
                <v-icon icon="mdi-wrench" size="small"></v-icon>
              </div>
              <span class="text-body-1 font-weight-medium">Toolchain</span>
              <v-chip size="x-small" class="ms-2" :color="settings.toolchain.enabled ? 'success' : 'default'" variant="tonal">
                {{ settings.toolchain.enabled ? 'Enabled' : 'Disabled' }}
              </v-chip>
              <v-chip v-if="configuredCounts.toolchain > 0" size="x-small" class="ms-2" color="success" variant="tonal">{{ badgeText('toolchain') }}</v-chip>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <div class="config-list">
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('toolchain.enabled')">
                <div class="config-label">Enabled</div>
                <div class="config-value-cell"><span :class="['boolean-badge', settings.toolchain.enabled ? 'enabled' : 'disabled']">{{ settings.toolchain.enabled ? 'true' : 'false' }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('toolchain.enabled', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">toolchain.enabled</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('toolchain.enabled', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_TOOLCHAIN__ENABLED</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('toolchain.enabled', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--toolchain-enabled</code></div>
                </div>
              </div>
              <div class="config-row" v-show="!showOnlyConfigured || isConfigured('toolchain.max_size')">
                <div class="config-label">Max Size (MB)</div>
                <div class="config-value-cell"><span class="value-text">{{ formatValue(settings.toolchain.max_size) }}</span></div>
                <div class="config-refs">
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('toolchain.max_size', 'toml') }"><span class="ref-badge toml">TOML</span><code class="ref-value">toolchain.max_size</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('toolchain.max_size', 'env') }"><span class="ref-badge env">ENV</span><code class="ref-value">KELLNR_TOOLCHAIN__MAX_SIZE</code></div>
                  <div class="config-ref" :class="{ 'source-active': isSourceActive('toolchain.max_size', 'cli') }"><span class="ref-badge cli">CLI</span><code class="ref-value">--toolchain-max-size</code></div>
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
import { onBeforeMount, ref, computed } from "vue";
import { emptySettings } from "../types/settings";
import type { Settings, ConfigSource } from "../types/settings";
import { settingsService } from "../services";
import { isSuccess } from "../services/api";

const settings = ref<Settings>(emptySettings);
const showOnlyConfigured = ref(false);

// Define setting keys for each section
const sectionKeys = {
  registry: [
    'registry.data_dir', 'registry.session_age_seconds', 'registry.cache_size',
    'registry.max_crate_size', 'registry.max_db_connections', 'registry.auth_required',
    'registry.required_crate_fields', 'registry.new_crates_restricted', 'registry.cookie_signing_key',
    'registry.allow_ownerless_crates', 'registry.token_cache_enabled', 'registry.token_cache_ttl_seconds',
    'registry.token_cache_max_capacity', 'registry.token_db_retry_count', 'registry.token_db_retry_delay_ms'
  ],
  local: ['local.ip', 'local.port'],
  origin: ['origin.hostname', 'origin.port', 'origin.protocol', 'origin.path'],
  log: ['log.level', 'log.format', 'log.level_web_server'],
  proxy: ['proxy.enabled', 'proxy.num_threads', 'proxy.download_on_update', 'proxy.url', 'proxy.index'],
  docs: ['docs.enabled', 'docs.max_size'],
  postgresql: ['postgresql.enabled', 'postgresql.address', 'postgresql.port', 'postgresql.db', 'postgresql.user'],
  s3: [
    's3.enabled', 's3.access_key', 's3.secret_key', 's3.region', 's3.endpoint',
    's3.allow_http', 's3.crates_bucket', 's3.cratesio_bucket', 's3.toolchain_bucket'
  ],
  toolchain: ['toolchain.enabled', 'toolchain.max_size']
};

// Computed properties for configured counts
const configuredCounts = computed(() => ({
  registry: sectionKeys.registry.filter(k => isConfigured(k)).length,
  local: sectionKeys.local.filter(k => isConfigured(k)).length,
  origin: sectionKeys.origin.filter(k => isConfigured(k)).length,
  log: sectionKeys.log.filter(k => isConfigured(k)).length,
  proxy: sectionKeys.proxy.filter(k => isConfigured(k)).length,
  docs: sectionKeys.docs.filter(k => isConfigured(k)).length,
  postgresql: sectionKeys.postgresql.filter(k => isConfigured(k)).length,
  s3: sectionKeys.s3.filter(k => isConfigured(k)).length,
  toolchain: sectionKeys.toolchain.filter(k => isConfigured(k)).length,
}));

// Check if a section should be visible
function isSectionVisible(section: keyof typeof sectionKeys): boolean {
  if (!showOnlyConfigured.value) return true;
  return configuredCounts.value[section] > 0;
}

// Format badge text showing configured/total
function badgeText(section: keyof typeof sectionKeys): string {
  const configured = configuredCounts.value[section];
  const total = sectionKeys[section].length;
  if (configured > 0) {
    return `${configured}/${total} configured`;
  }
  return `${total} settings`;
}

// Computed for total configured settings
const totalConfigured = computed(() => {
  return Object.values(configuredCounts.value).reduce((sum, count) => sum + count, 0);
});

// Computed for total settings
const totalSettings = computed(() => {
  return Object.values(sectionKeys).reduce((sum, keys) => sum + keys.length, 0);
});

// Computed for visible sections count
const visibleSectionsCount = computed(() => {
  if (!showOnlyConfigured.value) return 9;
  return Object.keys(sectionKeys).filter(section =>
    configuredCounts.value[section as keyof typeof sectionKeys] > 0
  ).length;
});

onBeforeMount(() => {
  getStartupConfig();
});

async function getStartupConfig() {
  const result = await settingsService.getSettings();
  if (isSuccess(result)) {
    settings.value = result.data;
  }
}

function formatValue(value: unknown): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return value.length > 0 ? value.join(', ') : '-';
  return String(value) || '-';
}

function getSource(key: string): ConfigSource {
  return settings.value.sources[key] || 'default';
}

function isConfigured(key: string): boolean {
  return getSource(key) !== 'default';
}

function isSourceActive(key: string, source: ConfigSource): boolean {
  return getSource(key) === source;
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
  margin-left: 8px;
  background: rgb(var(--v-theme-primary));
  color: rgb(var(--v-theme-on-primary));
  font-size: 12px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 12px;
}

.section-count:first-of-type {
  margin-left: auto;
}

.section-count.configured {
  background: rgb(var(--v-theme-success));
  color: rgb(var(--v-theme-on-success));
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

.ref-badge.cli {
  background: rgba(255, 152, 0, 0.15);
  color: #ff9800;
}

.ref-value {
  font-family: 'Roboto Mono', monospace;
  font-size: 12px;
  background: rgba(var(--v-theme-on-surface), 0.06);
  padding: 2px 6px;
  border-radius: 4px;
  color: rgb(var(--v-theme-on-surface-variant));
}

/* Source highlighting */
.config-ref {
  opacity: 0.5;
  transition: opacity 0.2s ease;
}

.config-ref.source-active {
  opacity: 1;
  font-weight: 600;
}

.config-ref.source-active .ref-badge {
  box-shadow: 0 0 0 2px currentColor;
}

.filter-toggle {
  display: flex;
  align-items: center;
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
