import React from 'react';
import { useTranslation } from 'react-i18next';

const Camera: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div className="container mx-auto p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
      <h1 className="text-2xl font-bold mb-4 text-gray-900 dark:text-white">{t('camera.title')}</h1>
      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <div className="relative w-full aspect-video bg-gray-900 rounded-lg overflow-hidden">
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="text-white text-lg">{t('camera.loading')}</div>
        </div>
      </div>
    </div>
  );
};

export default Camera; 