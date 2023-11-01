use ::settings::{Protocol, Settings};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct StartupSettings {
    pub data_dir: String,
    pub session_age_seconds: u64,
    pub api_address: String,
    pub api_port: u16,
    pub api_port_proxy: u16,
    pub api_protocol: Protocol,
    pub index_address: String,
    pub web_address: String,
    pub index_port: u16,
    pub crates_io_proxy: bool,
    pub crates_io_num_threads: usize,
    pub log_level: String,
    pub log_level_web_server: String,
    pub log_format: String,
    pub rustdoc_auto_gen: bool,
    pub cache_size: u64,
    pub postgresql: ::settings::Postgresql,
    pub max_crate_size: usize,
    pub max_docs_size: usize,
    pub auth_required: bool,
}

impl From<&Settings> for StartupSettings {
    fn from(settings: &Settings) -> Self {
        StartupSettings {
            data_dir: settings.data_dir.to_string(),
            session_age_seconds: settings.session_age_seconds,
            api_address: settings.api_address.to_string(),
            api_port: settings.api_port,
            api_port_proxy: settings.api_port_proxy,
            api_protocol: settings.api_protocol,
            index_address: settings.index_address.to_string(),
            web_address: settings.web_address.to_string(),
            index_port: settings.index_port,
            crates_io_proxy: settings.crates_io_proxy,
            crates_io_num_threads: settings.crates_io_num_threads,
            log_level: settings.log_level.to_string(),
            log_level_web_server: settings.log_level_web_server.to_string(),
            log_format: settings.log_format.to_string(),
            rustdoc_auto_gen: settings.rustdoc_auto_gen,
            cache_size: settings.cache_size,
            postgresql: settings.postgresql.clone(),
            max_crate_size: settings.max_crate_size,
            max_docs_size: settings.max_docs_size,
            auth_required: settings.auth_required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::settings::{LogFormat, Postgresql, Settings};

    #[test]
    fn settings_state_from_settings() {
        use std::net::{IpAddr, Ipv4Addr};
        let settings = Settings {
            data_dir: "data_dir".to_string(),
            admin_pwd: "admin_pwd".to_string(),
            session_age_seconds: 10,
            api_address: "api_address".to_string(),
            api_port: 123,
            api_port_proxy: 123,
            api_protocol: Protocol::default(),
            index_address: "index_address".to_string(),
            web_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            index_port: 312,
            admin_token: "admin_token".to_string(),
            crates_io_proxy: true,
            crates_io_num_threads: 10,
            log_level: tracing::Level::DEBUG,
            log_level_web_server: tracing::Level::WARN,
            log_format: LogFormat::Compact,
            rustdoc_auto_gen: true,
            cache_size: 100,
            max_crate_size: 100,
            max_docs_size: 100,
            auth_required: false,
            postgresql: Postgresql {
                enabled: false,
                address: "localhost".to_string(),
                port: 5432,
                db: "kellnr".to_string(),
                user: "user".to_string(),
                pwd: "pwd".to_string(),
            },
        };

        let state = StartupSettings::from(&settings);

        assert_eq!(state.data_dir, settings.data_dir);
        assert_eq!(state.session_age_seconds, settings.session_age_seconds);
        assert_eq!(state.api_address, settings.api_address);
        assert_eq!(state.api_port, settings.api_port);
        assert_eq!(state.api_protocol, settings.api_protocol);
        assert_eq!(state.index_address, settings.index_address);
        assert_eq!(state.web_address, settings.web_address.to_string());
        assert_eq!(state.index_port, settings.index_port);
        assert_eq!(state.crates_io_proxy, settings.crates_io_proxy);
        assert_eq!(state.log_level, settings.log_level.to_string());
        assert_eq!(
            state.log_level_web_server,
            settings.log_level_web_server.to_string()
        );
        assert_eq!(state.rustdoc_auto_gen, settings.rustdoc_auto_gen);
        assert_eq!(state.cache_size, settings.cache_size);
        assert_eq!(state.max_crate_size, settings.max_crate_size);
        assert_eq!(state.max_docs_size, settings.max_docs_size);
        assert_eq!(state.postgresql, settings.postgresql)
    }
}
