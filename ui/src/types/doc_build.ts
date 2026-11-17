export type DocBuildStatus = 'succeeded' | 'failed' | 'timed_out'

export type DocBuild = {
    id: number
    name: string
    version: string
    status: DocBuildStatus
    /** UTC date */
    started: string
    /** UTC date */
    finished: string
    /** Error output, only for admins and owners of the crate */
    error: string | null
    can_rebuild: boolean
}
