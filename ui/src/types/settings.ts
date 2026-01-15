export type Settings = {
  docs: Docs
  local: Local
  log: Log
  origin: Origin
  postgresql: Postgresql
  proxy: Proxy
  registry: Registry
  s3: S3
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
  allow_ownerless_crates: boolean
  token_cache_enabled: boolean
  token_cache_ttl_seconds: number
  token_cache_max_capacity: number
  token_db_retry_count: number
  token_db_retry_delay_ms: number
}

export type S3 = {
  enabled: boolean
  access_key: string
  secret_key: string
  region: string
  endpoint: string
  allow_http: boolean
  crates_bucket: string
  cratesio_bucket: string
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
    protocol: "0"
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
    download_on_update: false
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
    allow_ownerless_crates: false,
    token_cache_enabled: true,
    token_cache_ttl_seconds: 1800,
    token_cache_max_capacity: 10000,
    token_db_retry_count: 3,
    token_db_retry_delay_ms: 100
  },
  s3: {
    enabled: false,
    access_key: "",
    secret_key: "",
    region: "",
    endpoint: "",
    allow_http: false,
    crates_bucket: "",
    cratesio_bucket: ""
  }
}
