import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import StartupUpdate from './StartupUpdate'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <StartupUpdate />
  </StrictMode>,
)
