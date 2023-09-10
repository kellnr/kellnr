import { Store } from '@/store/store'

declare module '@vue/runtime-core' {
    interface ComponentCustomProperties {
        $store: Store
    }
}