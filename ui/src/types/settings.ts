export type Settings = {
    data_dir: string
    session_age_seconds: number
    api_address: string
    api_port: number
    api_port_proxy: number
    api_protocol: string
    index_address: string
    index_port: number
    web_address: string
    crates_io_proxy: boolean
    crates_io_num_threads: number
    log_level: string
    log_level_web_server: string
    log_format: string
    rustdoc_auto_gen: boolean
    max_crate_size: number
    max_docs_size: number
    cache_size: number
    auth_required: boolean
    postgresql: {
        enabled: boolean
        address: string
        port: number
        db: string
        user: string
    }
}

export const defaultSettings = {
    data_dir: "",
    session_age_seconds: 0,
    api_address: "",
    api_port: 0,
    api_port_proxy: 0,
    api_protocol: "",
    index_address: "",
    index_port: 0,
    web_address: "",
    crates_io_proxy: false,
    crates_io_num_threads: 0,
    log_level: "",
    log_level_web_server: "",
    log_format: "",
    rustdoc_auto_gen: false,
    max_crate_size: 0,
    max_docs_size: 0,
    cache_size: 0,
    auth_required: false,
    postgresql: {
        enabled: false,
        address: "",
        port: 0,
        db: "",
        user: "",
    }
}
