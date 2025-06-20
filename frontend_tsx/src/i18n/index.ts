import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import LanguageDetector from 'i18next-browser-languagedetector'

import de from './locales/de.json'
import en from './locales/en.json'

const savedLanguage = localStorage.getItem('language') || 'de'

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      de: {
        translation: de
      },
      en: {
        translation: en
      }
    },
    lng: savedLanguage,
    fallbackLng: 'de',
    interpolation: {
      escapeValue: false
    }
  })

i18n.on('languageChanged', (lng) => {
  localStorage.setItem('language', lng)
})

export default i18n 