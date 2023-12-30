export type Statistics = {
    num_crates: number,
    num_crate_versions: number,
    num_crate_downloads: number,
    num_proxy_crates: number,
    num_proxy_crate_versions: number,
    num_proxy_crate_downloads: number,
    top_crates: {
        first: [string, number],
        second: [string, number],
        third: [string, number],
    },
    last_updated_crate: [string, string], 
    proxy_enabled: boolean,
}
