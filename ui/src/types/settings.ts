export type Version = {
  version: string
}

export type DocsEnabled = {
  enabled: boolean
}

export type SponsorLinksEnabled = {
  enabled: boolean
}

export type RegistryConfig = {
  "auth-required": boolean
}

export type ConfigSource = 'default' | 'toml' | 'env' | 'cli';

export type SourceMap = Record<string, ConfigSource>;

// Type for default values (same structure as settings sections, no sources)
export type SettingsDefaults = {
  docs: Docs
  gcs: Gcs
  local: Local
  log: Log
  origin: Origin
  postgresql: Postgresql
  proxy: Proxy
  registry: Registry
  s3: S3
  toolchain: Toolchain
  webhooks: Webhooks
  cratesio_filter: CrateFilter
  registry_filter: CrateFilter
}

export type LeafKind = 'boolean' | 'number' | 'string' | 'array';

// Per-leaf metadata emitted by the backend. The StartupConfig view iterates
// over this list instead of hand-maintaining a parallel per-key table.
export type LeafMeta = {
  key: string
  value: unknown
  default: unknown
  source: ConfigSource
  type: LeafKind
  secret: boolean
  cli_flag?: string | null
  label?: string | null
}

export type Settings = {
  docs: Docs
  gcs: Gcs
  local: Local
  log: Log
  origin: Origin
  postgresql: Postgresql
  proxy: Proxy
  registry: Registry
  s3: S3
  toolchain: Toolchain
  webhooks: Webhooks
  cratesio_filter: CrateFilter
  registry_filter: CrateFilter
  sources: SourceMap
  defaults?: SettingsDefaults
  leaves?: LeafMeta[]
}

export type Toolchain = {
  enabled: boolean
  max_size: number
}

export type Webhooks = {
  block_private_targets: boolean
}

export type CrateFilter = {
  license_enabled: boolean
  license_allowed: string[]
  license_unknown: string
  license_exempt_crates: string[]
}

export type Docs = {
  enabled: boolean
  max_size: number
}

export type Local = {
  ip: string
  port: number
}

export type Log = {
  level: string
  format: string
  level_web_server: string
}

export type Origin = {
  hostname: string
  port: number
  protocol: string
  path: string
  cookie_secure: boolean | null
}

export type Postgresql = {
  enabled: boolean
  address: string
  port: number
  db: string
  user: string
}

export type Proxy = {
  enabled: boolean
  num_threads: number
  download_on_update: boolean
  url: string
  index: string
  api: string
  connect_timeout_seconds: number
  request_timeout_seconds: number
}

export type Registry = {
  data_dir: string
  session_age_seconds: number
  cache_size: number
  cache_max_size: number
  max_crate_size: number
  max_db_connections: number
  auth_required: boolean
  required_crate_fields: string[]
  new_crates_restricted: boolean
  cookie_signing_key: string | null
  allow_ownerless_crates: boolean
  token_cache_enabled: boolean
  token_cache_ttl_seconds: number
  token_cache_max_capacity: number
  token_db_retry_count: number
  token_db_retry_delay_ms: number
  download_timeout_seconds: number
  download_max_concurrent: number
  download_counter_flush_seconds: number
  auth_max_failed_attempts: number
  auth_failed_attempts_window_seconds: number
  sponsor_links_enabled: boolean
}

export type S3 = {
  enabled: boolean
  access_key: string | null
  secret_key: string | null
  region: string | null
  endpoint: string | null
  allow_http: boolean
  crates_bucket: string
  cratesio_bucket: string
  toolchain_bucket: string
  docs_bucket: string | null
  connect_timeout_seconds: number
  request_timeout_seconds: number
}

export type Gcs = {
  enabled: boolean
  endpoint: string | null
  allow_http: boolean
  skip_signature: boolean
  crates_bucket: string
  cratesio_bucket: string
  toolchain_bucket: string
  docs_bucket: string | null
  connect_timeout_seconds: number
  request_timeout_seconds: number
}

export const emptySettings: Settings = {
  docs: {
    enabled: true,
    max_size: 0
  },
  gcs: {
    enabled: false,
    endpoint: null,
    allow_http: false,
    skip_signature: false,
    crates_bucket: "",
    cratesio_bucket: "",
    toolchain_bucket: "",
    docs_bucket: null,
    connect_timeout_seconds: 5,
    request_timeout_seconds: 30
  },
  local: {
    ip: "",
    port: 0
  },
  log: {
    level: "",
    format: "",
    level_web_server: ""
  },
  origin: {
    hostname: "",
    port: 0,
    protocol: "0",
    path: "",
    cookie_secure: null
  },
  postgresql: {
    enabled: false,
    address: "",
    port: 0,
    db: "",
    user: ""
  },
  proxy: {
    enabled: false,
    num_threads: 0,
    download_on_update: false,
    url: "",
    index: "",
    api: "",
    connect_timeout_seconds: 5,
    request_timeout_seconds: 30
  },
  registry: {
    data_dir: "",
    session_age_seconds: 0,
    cache_size: 0,
    cache_max_size: 0,
    max_crate_size: 0,
    max_db_connections: 0,
    auth_required: false,
    required_crate_fields: [],
    new_crates_restricted: false,
    cookie_signing_key: null,
    allow_ownerless_crates: false,
    token_cache_enabled: true,
    token_cache_ttl_seconds: 1800,
    token_cache_max_capacity: 10000,
    token_db_retry_count: 3,
    token_db_retry_delay_ms: 100,
    download_timeout_seconds: 60,
    download_max_concurrent: 20,
    download_counter_flush_seconds: 30,
    auth_max_failed_attempts: 10,
    auth_failed_attempts_window_seconds: 300,
    sponsor_links_enabled: true
  },
  s3: {
    enabled: false,
    access_key: null,
    secret_key: null,
    region: null,
    endpoint: null,
    allow_http: false,
    crates_bucket: "",
    cratesio_bucket: "",
    toolchain_bucket: "",
    docs_bucket: null,
    connect_timeout_seconds: 5,
    request_timeout_seconds: 30
  },
  toolchain: {
    enabled: false,
    max_size: 500
  },
  webhooks: {
    block_private_targets: false
  },
  cratesio_filter: {
    license_enabled: false,
    license_allowed: [],
    license_unknown: "deny",
    license_exempt_crates: []
  },
  registry_filter: {
    license_enabled: false,
    license_allowed: [],
    license_unknown: "deny",
    license_exempt_crates: []
  },
  sources: {},
  defaults: undefined,
  leaves: []
}
