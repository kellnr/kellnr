/**
 * Error bodies of the Kellnr API have the format of the cargo registry API:
 * `{"errors":[{"detail":"ERROR: ..."}]}`.
 */
interface ErrorBody {
  errors?: { detail?: unknown }[]
}

const DETAIL_PREFIX = 'ERROR: '

/**
 * The message of an error response body, without the `ERROR: ` prefix that
 * cargo shows. `undefined` if the body has no message.
 */
export function errorDetail(body: unknown): string | undefined {
  const detail = (body as ErrorBody | null | undefined)?.errors?.[0]?.detail
  if (typeof detail !== 'string') {
    return undefined
  }
  const message = detail.startsWith(DETAIL_PREFIX) ? detail.slice(DETAIL_PREFIX.length) : detail
  return message.length > 0 ? message : undefined
}
