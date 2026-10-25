/**
 * Composable for relative times like "5 minutes ago" that stay current
 *
 * Usage:
 * ```ts
 * const { fromNow, exactDate } = useRelativeTime()
 * const humanized = computed(() => fromNow(props.updated))
 * ```
 */
import { ref } from 'vue'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import utc from 'dayjs/plugin/utc'

dayjs.extend(relativeTime)
dayjs.extend(utc)

const TICK_MS = 30_000

// One clock for all components, so every relative time on a page is computed
// from the same moment and they never disagree.
const now = ref(dayjs())
let timer: ReturnType<typeof setInterval> | undefined

function startClock() {
  if (timer === undefined) {
    timer = setInterval(() => {
      now.value = dayjs()
    }, TICK_MS)
  }
}

export interface UseRelativeTimeReturn {
  /** Time since a UTC date from the API, e.g. "5 minutes ago" */
  fromNow: (utcDate: string) => string
  /** A UTC date from the API in the local time zone, e.g. "2026-10-25 11:43" */
  exactDate: (utcDate: string) => string
}

export function useRelativeTime(): UseRelativeTimeReturn {
  startClock()

  return {
    fromNow: (utcDate: string) => dayjs.utc(utcDate).from(now.value),
    exactDate: (utcDate: string) => dayjs.utc(utcDate).local().format('YYYY-MM-DD HH:mm'),
  }
}
