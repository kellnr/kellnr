import { nextTick, onBeforeUnmount, onMounted } from "vue"
import { useRouter } from "vue-router"

// Search fields opt in with the data-search-shortcut attribute
const SEARCH_FIELD = "[data-search-shortcut] input, input[data-search-shortcut]"
const SHORTCUT_KEYS = ["/", "s", "S"]

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
}

function focusSearchField(): boolean {
  const field = document.querySelector<HTMLInputElement>(SEARCH_FIELD)
  if (!field) return false
  field.focus()
  field.select()
  return true
}

/**
 * Focuses the search field when "/" or "S" is pressed, like on crates.io.
 * Pages without a search field open the crate list.
 */
export function useSearchShortcut() {
  const router = useRouter()

  function onKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return
    if (!SHORTCUT_KEYS.includes(event.key) || isEditable(event.target)) return
    // Leave open dialogs alone, the search field is behind them
    if (document.querySelector(".v-dialog.v-overlay--active")) return

    event.preventDefault()
    if (!focusSearchField()) {
      router.push({ name: "Crates" }).then(() => nextTick(focusSearchField))
    }
  }

  onMounted(() => window.addEventListener("keydown", onKeydown))
  onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown))
}
