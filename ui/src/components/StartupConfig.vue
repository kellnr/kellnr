<template>
  <v-container>
    <v-card-title class="text-h4 pa-4">Startup Config</v-card-title>

    <v-card-text>
      <p class="mb-6">
        Values are set on application startup. See
        <a href="https://kellnr.io/documentation" class="text-primary">Kellnr Configuration Documentation</a>
        for more information.
      </p>

      <!-- Registry Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-package-variant-closed" class="mr-2"></v-icon>
              <span class="text-h5">Registry</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Data Directory</td>
                  <td>{{ formatValue(settings.registry.data_dir) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.data_dir</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__DATA_DIR</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Session Age (seconds)</td>
                  <td>{{ formatValue(settings.registry.session_age_seconds) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.session_age_seconds</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__SESSION_AGE_SECONDS</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Cache Size</td>
                  <td>{{ formatValue(settings.registry.cache_size) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.cache_size</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__CACHE_SIZE</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Max Crate Size</td>
                  <td>{{ formatValue(settings.registry.max_crate_size) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.max_crate_size</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__MAX_CRATE_SIZE</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Max DB Connections</td>
                  <td>{{ formatValue(settings.registry.max_db_connections) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.max_db_connections</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__MAX_DB_CONNECTIONS</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Auth Required</td>
                  <td>
                    <v-chip :color="settings.registry.auth_required ? 'success' : 'grey'" size="small"
                      text-color="white">
                      {{ settings.registry.auth_required ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.auth_required</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__AUTH_REQUIRED</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Required Crate Fields</td>
                  <td>{{ formatValue(settings.registry.required_crate_fields) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.required_crate_fields</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__REQUIRED_CRATE_FIELDS</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">New crates restricted</td>
                  <td>{{ formatValue(settings.registry.new_crates_restricted) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">registry.new_crates_restricted</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_REGISTRY__NEW_CRATES_RESTRICTED</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- Local Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-server" class="mr-2"></v-icon>
              <span class="text-h5">Local</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">IP</td>
                  <td>{{ formatValue(settings.local.ip) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">local.ip</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_LOCAL__IP</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Port</td>
                  <td>{{ formatValue(settings.local.port) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">local.port</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_LOCAL__PORT</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- Origin Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-earth" class="mr-2"></v-icon>
              <span class="text-h5">Origin</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Hostname</td>
                  <td>{{ formatValue(settings.origin.hostname) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">origin.hostname</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_ORIGIN__HOSTNAME</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Port</td>
                  <td>{{ formatValue(settings.origin.port) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">origin.port</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_ORIGIN__PORT</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Protocol</td>
                  <td>{{ formatValue(settings.origin.protocol) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">origin.protocol</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_ORIGIN__PROTOCOL</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- Log Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-file-document-outline" class="mr-2"></v-icon>
              <span class="text-h5">Log</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Level</td>
                  <td>{{ formatValue(settings.log.level) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">log.level</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_LOG__LEVEL</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Format</td>
                  <td>{{ formatValue(settings.log.format) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">log.format</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_LOG__FORMAT</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Level Web Server</td>
                  <td>{{ formatValue(settings.log.level_web_server) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">log.level_web_server</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_LOG__LEVEL_WEB_SERVER</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- Proxy Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-transit-connection-variant" class="mr-2"></v-icon>
              <span class="text-h5">Proxy</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Enabled</td>
                  <td>
                    <v-chip :color="settings.proxy.enabled ? 'success' : 'grey'" size="small" text-color="white">
                      {{ settings.proxy.enabled ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">proxy.enabled</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_PROXY__ENABLED</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Number of Threads</td>
                  <td>{{ formatValue(settings.proxy.num_threads) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">proxy.num_threads</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_PROXY__NUM_THREADS</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Download on Update</td>
                  <td>
                    <v-chip :color="settings.proxy.download_on_update ? 'success' : 'grey'" size="small" text-color="white">
                      {{ settings.proxy.download_on_update ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">proxy.download_on_update</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_PROXY__DOWNLOAD_ON_UPDATE</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- Docs Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-file-document-multiple-outline" class="mr-2"></v-icon>
              <span class="text-h5">Docs</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Enabled</td>
                  <td>
                    <v-chip :color="settings.docs.enabled ? 'success' : 'grey'" size="small" text-color="white">
                      {{ settings.docs.enabled ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">docs.enabled</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_DOCS__ENABLED</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Max Size</td>
                  <td>{{ formatValue(settings.docs.max_size) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">docs.max_size</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_DOCS__MAX_SIZE</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- PostgreSQL Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-database" class="mr-2"></v-icon>
              <span class="text-h5">PostgreSQL</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Enabled</td>
                  <td>
                    <v-chip :color="settings.postgresql.enabled ? 'success' : 'grey'" size="small" text-color="white">
                      {{ settings.postgresql.enabled ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">postgresql.enabled</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_POSTGRESQL__ENABLED</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Address</td>
                  <td>{{ formatValue(settings.postgresql.address) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">postgresql.address</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_POSTGRESQL__ADDRESS</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Port</td>
                  <td>{{ formatValue(settings.postgresql.port) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">postgresql.port</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_POSTGRESQL__PORT</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Database</td>
                  <td>{{ formatValue(settings.postgresql.db) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">postgresql.db</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_POSTGRESQL__DB</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">User</td>
                  <td>{{ formatValue(settings.postgresql.user) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">postgresql.user</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_POSTGRESQL__USER</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>

      <!-- S3 Section -->
      <v-expansion-panels variant="accordion" class="mb-6">
        <v-expansion-panel>
          <v-expansion-panel-title>
            <div class="d-flex align-center">
              <v-icon icon="mdi-cloud-outline" class="mr-2"></v-icon>
              <span class="text-h5">S3</span>
            </div>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-table density="compact">
              <thead>
                <tr>
                  <th class="text-left" width="20%">Configuration Key</th>
                  <th class="text-left" width="20%">Value</th>
                  <th class="text-left" width="60%">Config Reference</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td class="text-subtitle-2">Enabled</td>
                  <td>
                    <v-chip :color="settings.s3.enabled ? 'success' : 'grey'" size="small" text-color="white">
                      {{ settings.s3.enabled ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">s3.enabled</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_S3__ENABLED</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Access Key</td>
                  <td>{{ formatValue(settings.s3.access_key) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">s3.access_key</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_S3__ACCESS_KEY</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Secret Key</td>
                  <td>{{ settings.s3.secret_key ? '********' : '-' }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">s3.secret_key</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_S3__SECRET_KEY</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Region</td>
                  <td>{{ formatValue(settings.s3.region) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">s3.region</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_S3__REGION</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Endpoint</td>
                  <td>{{ formatValue(settings.s3.endpoint) }}</td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">s3.endpoint</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_S3__ENDPOINT</span>
                    </div>
                  </td>
                </tr>
                <tr>
                  <td class="text-subtitle-2">Allow HTTP</td>
                  <td>
                    <v-chip :color="settings.s3.allow_http ? 'warning' : 'grey'" size="small" text-color="white">
                      {{ settings.s3.allow_http ? 'true' : 'false' }}
                    </v-chip>
                  </td>
                  <td>
                    <div class="config-ref">
                      <v-chip label size="small" color="deep-purple-darken-1" text-color="white"
                        class="config-chip">TOML</v-chip>
                      <span class="config-value">s3.allow_http</span>
                    </div>
                    <div class="config-ref">
                      <v-chip label size="small" color="teal-darken-1" text-color="white"
                        class="config-chip">ENV</v-chip>
                      <span class="config-value">KELLNR_S3__ALLOW_HTTP</span>
                    </div>
                  </td>
                </tr>
              </tbody>
            </v-table>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
    </v-card-text>
  </v-container>
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
  if (Array.isArray(value)) return value.join(', ');
  return value.toString();
}
</script>

<style scoped>
.config-ref {
  display: flex;
  align-items: center;
  margin-bottom: 4px;
}

.config-chip {
  min-width: 55px;
  font-size: 11px;
  margin-right: 8px;
}

.config-value {
  font-family: "Roboto Mono", monospace;
  font-size: 13px;
  background-color: rgba(0, 0, 0, 0.04);
  padding: 2px 6px;
  border-radius: 4px;
}
</style>
