import { useState, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { BrowserRouter as Router, Routes, Route, Link } from 'react-router-dom'
import Header from './components/Header'
import Footer from './components/Footer'
import TerrariumController from './pages/TerrariumController'
import Schedule from './pages/Schedule'
import DataPage from './pages/DataPage'
import LEDControl from './pages/LEDControl'
import Camera from './pages/Camera'

function App() {
  const { i18n } = useTranslation()
  const [darkMode, setDarkMode] = useState(() => {
    const savedMode = localStorage.getItem('darkMode')
    return savedMode ? JSON.parse(savedMode) : false
  })

  useEffect(() => {
    if (darkMode) {
      document.documentElement.classList.add('dark')
    } else {
      document.documentElement.classList.remove('dark')
    }
    localStorage.setItem('darkMode', JSON.stringify(darkMode))
  }, [darkMode])

  const toggleDarkMode = () => {
    setDarkMode(!darkMode)
  }

  const changeLanguage = (lng: string) => {
    i18n.changeLanguage(lng)
  }

  return (
    <Router>
      <div className="min-h-screen bg-[#999] dark:bg-gray-900 transition-colors duration-200">
        <Header 
          darkMode={darkMode} 
          toggleDarkMode={toggleDarkMode}
          changeLanguage={changeLanguage}
        />
        <main className="container mx-auto px-4 py-8">
          <Routes>
            <Route path="/" element={<TerrariumController />} />
            <Route path="/schedule" element={<Schedule />} />
            <Route path="/data" element={<DataPage />} />
            <Route path="/led" element={<LEDControl />} />
            <Route path="/cam" element={<Camera />} />
          </Routes>
        </main>
        <Footer />
      </div>
    </Router>
  )
}

export default App 