export type CrateData = {
    name: string,
    owners: Array<string>,
    max_version: string,
    total_downloads: number,
    last_updated: string,
    homepage?: string,
    description?: string,
    repository?: string,
    categories: Array<string>,
    keywords: Array<string>,
    authors: Array<string>,
    versions: Array<CrateVersionData>,
}

export const defaultCrateData : CrateData = {
    name: "",
    owners: [],
    max_version: "",
    total_downloads: 0,
    last_updated: "",
    homepage: "",
    description: "",
    repository: "",
    categories: [],
    keywords: [],
    authors: [],
    versions: [],
}

export type CrateVersionData = {
    version: string,
    created: string,
    downloads: number,
    readme?: string,
    license?: string,
    license_file?: string,
    documentation?: string,
    dependencies: Array<CrateRegistryDep>,
    checksum: string,
    features: { [key: string]: Array<string> },
    features2?: { [key: string]: Array<string> },
    yanked: boolean,
    links?: string,
    v: number,
    /** Size of the .crate file in bytes, 0 if unknown */
    size: number,
    /** Minimal supported Rust version */
    rust_version?: string,
    /** User who published the version, unknown for older versions */
    publisher?: string,
    /** Name of the auth token used to publish, unknown for basic auth */
    publisher_token?: string,
}

export const defaultCrateVersionData : CrateVersionData = {
    version: "",
    created: "",
    downloads: 0,
    readme: "",
    license: "",
    license_file: "",
    documentation: "",
    dependencies: [],
    checksum: "",
    features: {},
    features2: undefined,
    yanked: false,
    links: "",
    v: 0,
    size: 0,
    rust_version: undefined,
    publisher: undefined,
    publisher_token: undefined,
}

export type CrateRegistryDep = {
    name: string,
    description?: string,
    version_req: string,
    features?: Array<string>,
    optional: boolean,
    default_features: boolean,
    target?: string,
    kind?: string,
    registry?: string,
    explicit_name_in_toml?: string,
}

export type CrateAccessData = {
    is_download_restricted: boolean,
}

export const defaultCrateAccessData : CrateAccessData = {
    is_download_restricted: true,
}

export interface CrateGroup {
    name: string;
}
