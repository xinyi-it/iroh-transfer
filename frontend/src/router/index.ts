import { createRouter, createWebHashHistory } from 'vue-router'
import TransferView from '../views/TransferView.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'transfer',
      component: TransferView
    }
  ]
})

export default router
