import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.tsx'
import './index.css'
import { initializePlatformStyles } from './lib/platformStyles'

// Initialize platform-specific styles before rendering
console.log('🎨 Initializing platform-specific styles...');
initializePlatformStyles()
console.log('✅ Platform styles initialized');

console.log('🚀 Starting React application...');
ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
console.log('✅ React application started');
