export type VersionInfo = {
    version: string
}

export function defaultVersionInfo(): VersionInfo {
    return {
        version: 'unknown',
    }
}
