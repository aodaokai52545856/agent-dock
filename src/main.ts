import { createApp } from 'vue'
import App from './App.vue'
import { applyHostOsClass } from './lib/platform'
import './styles/global.css'

applyHostOsClass()
createApp(App).mount('#app')
