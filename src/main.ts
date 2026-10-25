import { createApp } from 'vue'
import App from './App.vue'
import { applyHostOsClass } from './lib/platform'
import { modal } from './lib/modal'
import './styles/global.css'

applyHostOsClass()
createApp(App).directive('modal', modal).mount('#app')
