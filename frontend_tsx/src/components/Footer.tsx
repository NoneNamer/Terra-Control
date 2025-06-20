import { useTranslation } from 'react-i18next'

const Footer = () => {
  const { t } = useTranslation()

  return (
    <footer className="bg-white dark:bg-gray-800 shadow-md mt-8">
      <div className="container mx-auto px-4 py-6">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
          <div>
            <h3 className="text-lg font-semibold text-gray-800 dark:text-white mb-4">
              {t('footer.about')}
            </h3>
            <p className="text-gray-600 dark:text-gray-300">
              {t('footer.aboutText')}
            </p>
          </div>
          
          <div>
            <h3 className="text-lg font-semibold text-gray-800 dark:text-white mb-4">
              {t('footer.contact')}
            </h3>
            <p className="text-gray-600 dark:text-gray-300">
              Email: info@void-network.de<br />
              Tel: Auf Anfrage
            </p>
          </div>
          
          <div>
            <h3 className="text-lg font-semibold text-gray-800 dark:text-white mb-4">
              {t('footer.follow')}
            </h3>
            <div className="flex space-x-4">
              <a href="#" className="text-gray-600 dark:text-gray-300 hover:text-gray-800 dark:hover:text-white">
                Twitter
              </a>
              <a href="#" className="text-gray-600 dark:text-gray-300 hover:text-gray-800 dark:hover:text-white">
                LinkedIn
              </a>
              <a href="https://github.com/NoneNamer/Terra-Control" className="text-gray-600 dark:text-gray-300 hover:text-gray-800 dark:hover:text-white">
                GitHub
              </a>
            </div>
          </div>
        </div>
        
        <div className="mt-8 pt-8 border-t border-gray-200 dark:border-gray-700">
          <p className="text-center text-gray-600 dark:text-gray-300">
            © Void-Network {new Date().getFullYear()} {t('footer.copyright')}
          </p>
        </div>
      </div>
    </footer>
  )
}

export default Footer 