export type CrateOverview = {
    name: string
    version: string
    date: string
    total_downloads: number
    description?: string
    documentation?: string
    is_kellnr: boolean
    /** Alias for !is_kellnr - indicates if this is from crates.io cache */
    is_cache?: boolean
}
