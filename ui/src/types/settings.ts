export type Version = {
  version: string
}

export type DocsEnabled = {
  enabled: boolean
}

export type ConfigSource = 'default' | 'toml' | 'env' | 'cli';

export type SourceMap = Record<string, ConfigSource>;

// Type for default values (same structure as settings sections, no sources)
export type SettingsDefaults = {
  docs: Docs
  local: Local
  log: Log
  origin: Origin
  postgresql: Postgresql
  proxy: Proxy
  registry: Registry
  storage: Storage
  toolchain: Toolchain
}

export type Settings = {
  docs: Docs
  local: Local
  log: Log
  origin: Origin
  postgresql: Postgresql
  proxy: Proxy
  registry: Registry
  storage: Storage
  toolchain: Toolchain
  sources: SourceMap
  defaults?: SettingsDefaults
}

export type Toolchain = {
  enabled: boolean
  max_size: number
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
}

export type Registry = {
  data_dir: string
  session_age_seconds: number
  cache_size: number
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
}

export type Storage = {
  kellnr_crates: StorageBackend | null
  crates_io: StorageBackend | null
  toolchain: StorageBackend | null
}

export type StorageBackend = FileBackend | S3Backend;

export type FileBackend = {
  kind: 'file'
  folder: string
}

export type S3Backend = {
  kind: 's3'
  bucket: string
  access_key: string | null
  secret_key: string | null
  region: string | null
  endpoint: string | null
  allow_http: boolean
}

export const emptySettings: Settings = {
  docs: {
    enabled: true,
    max_size: 0
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
    path: ""
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
    index: ""
  },
  registry: {
    data_dir: "",
    session_age_seconds: 0,
    cache_size: 0,
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
    token_db_retry_delay_ms: 100
  },
  storage: {
    kellnr_crates: {
      kind: 'file',
      folder: 'crates'
    },
    crates_io: {
      kind: 'file',
      folder: 'crates-io'
    },
    toolchain: null
  },
  toolchain: {
    enabled: false,
    max_size: 500
  },
  sources: {},
  defaults: undefined
}
