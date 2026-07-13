import { createRouter, createWebHashHistory } from 'vue-router'
import TransferView from '../views/TransferView.vue'
import AboutView from '../views/AboutView.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'transfer',
      component: TransferView
    },
    {
      path: '/about',
      name: 'about',
      component: AboutView
    }
  ]
})

export default router
