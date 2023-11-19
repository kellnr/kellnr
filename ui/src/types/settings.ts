export type Settings = {
    docs: Docs
    local: Local
    log: Log
    origin: Origin
    postgresql: Postgresql
    proxy: Proxy
    registry: Registry
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
}

export type Registry = {
    data_dir: string
    session_age_seconds: number
    cache_size: number
    max_crate_size: number
    auth_required: boolean
}

export const emptySettings = {
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
        num_threads: 0
    },
    registry: {
        data_dir: "",
        session_age_seconds: 0,
        cache_size: 0,
        max_crate_size: 0,
        auth_required: false,
    },
}
