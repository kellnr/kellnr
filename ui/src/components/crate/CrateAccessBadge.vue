<template>
  <button v-if="restricted" type="button" class="access-badge" :class="{ 'access-badge--warning': !authRequired }"
    data-testid="crate-access-badge" :data-auth-required="authRequired" :title="title" @click="emit('open')">
    <v-icon :icon="authRequired ? 'mdi-lock-outline' : 'mdi-alert-outline'" size="x-small" />
    <span>Restricted: {{ summary }}</span>
  </button>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { crateService, settingsService } from '../../services'
import { isSuccess } from '../../services/api'
import { formatCount } from '../../common/format'

const props = defineProps<{
  crateName: string
  // Changes whenever the access settings change, to load them again
  refreshKey: number
}>()

const emit = defineEmits<{
  (e: 'open'): void
}>()

const restricted = ref(false)
const userCount = ref(0)
const groupCount = ref(0)
// Cargo sends credentials only if the registry requires them. Without them,
// the crate users cannot download a restricted crate with cargo either.
const authRequired = ref(true)

const title = computed(() => authRequired.value
  ? 'Downloads are restricted. Open the access settings.'
  : 'Crate users cannot download this crate with cargo, because auth_required is off. Set auth_required = true in the Kellnr configuration.')

const summary = computed(() => `${formatCount(userCount.value, 'user')}, ${formatCount(groupCount.value, 'group')}`)

async function load() {
  const name = props.crateName
  const [access, users, groups, config] = await Promise.all([
    crateService.getCrateAccessData(name),
    crateService.getCrateUsers(name),
    crateService.getCrateGroups(name),
    settingsService.getRegistryConfig(),
  ])
  // Another crate was opened while loading
  if (name !== props.crateName) return

  restricted.value = isSuccess(access) && access.data.download_restricted
  userCount.value = isSuccess(users) ? users.data.users?.length ?? 0 : 0
  groupCount.value = isSuccess(groups) ? groups.data.groups?.length ?? 0 : 0
  // Only warn if the registry says so, not if the config failed to load
  authRequired.value = !isSuccess(config) || config.data['auth-required']
}

onMounted(load)
watch(() => [props.crateName, props.refreshKey], load)
</script>

<style scoped>
.access-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 10px;
  border-radius: 9999px;
  font-size: 13px;
  line-height: 1.6;
  cursor: pointer;
  /* The warning color is too light for text in the light theme */
  color: rgb(var(--v-theme-on-surface));
  background: rgba(var(--v-theme-warning), 0.12);
  border: 1px solid rgba(var(--v-theme-warning), 0.5);
  transition: border-color 0.2s ease;
}

.access-badge .v-icon {
  color: rgb(var(--v-theme-warning));
}

.access-badge--warning,
.access-badge--warning:hover,
.access-badge--warning:focus-visible {
  border-color: rgb(var(--v-theme-warning));
}

.access-badge:hover,
.access-badge:focus-visible {
  border-color: rgb(var(--v-theme-warning));
}
</style>
