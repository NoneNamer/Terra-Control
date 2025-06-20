import { useTranslation } from 'react-i18next'
import { SunIcon, MoonIcon } from '@heroicons/react/24/outline'
import { useLocation } from 'react-router-dom'

interface HeaderProps {
  darkMode: boolean
  toggleDarkMode: () => void
  changeLanguage: (lng: string) => void
}

const Header = ({ darkMode, toggleDarkMode, changeLanguage }: HeaderProps) => {
  const { t } = useTranslation()
  const location = useLocation()

  const isActive = (path: string) => {
    return location.pathname === path
  }

  return (
    <header className="bg-white dark:bg-gray-800 shadow-md">
      <nav className="container mx-auto px-4 py-4">
        <div className="flex justify-between items-center">
          <div className="flex items-center space-x-4">
            <h1 className="text-xl font-bold text-gray-800 dark:text-white">
              {t('header.title')}
            </h1>
            <div className="flex gap-4">
              <a 
                href="/" 
                className={`px-3 py-2 rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-all duration-200 ${
                  isActive('/') ? 'border-b-2 border-blue-500' : ''
                }`}
              >
                {t('terrarium.title')}
              </a>
              <a 
                href="/schedule" 
                className={`px-3 py-2 rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-all duration-200 ${
                  isActive('/schedule') ? 'border-b-2 border-blue-500' : ''
                }`}
              >
                {t('schedule.title')}
              </a>
              <a 
                href="/data" 
                className={`px-3 py-2 rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-all duration-200 ${
                  isActive('/data') ? 'border-b-2 border-blue-500' : ''
                }`}
              >
                {t('data.title')}
              </a>
              <a 
                href="/led" 
                className={`px-3 py-2 rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-all duration-200 ${
                  isActive('/led') ? 'border-b-2 border-blue-500' : ''
                }`}
              >
                {t('led.title')}
              </a>
              <a 
                href="/cam" 
                className={`px-3 py-2 rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-all duration-200 ${
                  isActive('/cam') ? 'border-b-2 border-blue-500' : ''
                }`}
              >
                {t('camera.title')}
              </a>
            </div>
          </div>
          
          <div className="flex items-center space-x-4">
            <button
              onClick={() => changeLanguage('de')}
              className="px-3 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-white"
            >
              DE
            </button>
            <button
              onClick={() => changeLanguage('en')}
              className="px-3 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-white"
            >
              EN
            </button>
            <button
              onClick={toggleDarkMode}
              className="p-2 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-white"
            >
              {darkMode ? (
                <SunIcon className="h-5 w-5" />
              ) : (
                <MoonIcon className="h-5 w-5" />
              )}
            </button>
          </div>
        </div>
      </nav>
    </header>
  )
}

export default Header 