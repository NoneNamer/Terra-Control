import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import '../styles/DataPage.css';

interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

const DataPage = () => {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState('all');
  const [limit, setLimit] = useState(50);
  const [startDate, setStartDate] = useState<string>('');
  const [endDate, setEndDate] = useState<string>('');

  useEffect(() => {
    // Set default date range (last 30 days)
    const today = new Date();
    const thirtyDaysAgo = new Date();
    thirtyDaysAgo.setDate(today.getDate() - 30);
    
    setEndDate(today.toISOString().split('T')[0]);
    setStartDate(thirtyDaysAgo.toISOString().split('T')[0]);
    
    loadLogEntries();
  }, []);

  const loadLogEntries = async () => {
    try {
      // TODO: Implement API call
      const response = await fetch(`/api/logs?filter=${filter}&limit=${limit}`);
      const data = await response.json();
      setLogs(data);
    } catch (error) {
      console.error('Error loading logs:', error);
    }
  };

  const downloadLogs = async () => {
    try {
      // TODO: Implement API call
      const response = await fetch('/api/logs/download');
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'logs.zip';
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);
    } catch (error) {
      console.error('Error downloading logs:', error);
    }
  };

  const downloadSensorData = async () => {
    if (!startDate || !endDate) {
      alert('Please select both start and end dates');
      return;
    }

    try {
      // TODO: Implement API call
      const response = await fetch(`/api/sensor-data/download?start=${startDate}&end=${endDate}`);
      const blob = await response.blob();
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'sensor-data.csv';
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);
    } catch (error) {
      console.error('Error downloading sensor data:', error);
    }
  };

  return (
    <div className="container mx-auto p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
      <h1 className="text-2xl font-bold mb-4 text-gray-900 dark:text-white">{t('data.title')}</h1>
      <hr className="my-4 border-gray-300 dark:border-gray-600" />
      
      <div className="mb-8">
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('data.recentLogs')}</h2>
        <div className="bg-white dark:bg-gray-700 p-4 rounded-lg">
          <div className="space-y-2">
            {logs.length === 0 ? (
              <p className="text-gray-700 dark:text-gray-300">{t('data.loadingLogs')}</p>
            ) : (
              logs.map((log, index) => (
                <div key={index} className={`p-2 rounded ${log.level.toLowerCase()} text-gray-700 dark:text-gray-300`}>
                  <span className="font-mono mr-2">
                    {new Date(log.timestamp).toLocaleString()}
                  </span>
                  <span className="font-semibold mr-2">{log.level}</span>
                  <span>{log.message}</span>
                </div>
              ))
            )}
          </div>
          <div className="mt-4 flex gap-4">
            <button 
              onClick={loadLogEntries} 
              className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
            >
              {t('data.refreshLogs')}
            </button>
            <select
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              className="px-4 py-2 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
            >
              <option value="all">{t('data.allEntries')}</option>
              <option value="info">{t('data.info')}</option>
              <option value="warning">{t('data.warnings')}</option>
              <option value="error">{t('data.errors')}</option>
            </select>
            <input
              type="number"
              min="10"
              max="500"
              value={limit}
              onChange={(e) => setLimit(Number(e.target.value))}
              placeholder={t('data.limit')}
              className="px-4 py-2 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
            />
          </div>
        </div>
      </div>

      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <div>
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('data.downloadData')}</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="bg-white dark:bg-gray-700 p-4 rounded-lg">
            <h3 className="text-lg font-semibold mb-2 text-gray-800 dark:text-gray-100">{t('data.logFiles')}</h3>
            <p className="text-gray-700 dark:text-gray-300 mb-4">{t('data.downloadSystemLogFiles')}</p>
            <button 
              onClick={downloadLogs} 
              className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
            >
              {t('data.downloadLogs')}
            </button>
          </div>
          <div className="bg-white dark:bg-gray-700 p-4 rounded-lg">
            <h3 className="text-lg font-semibold mb-2 text-gray-800 dark:text-gray-100">{t('data.sensorData')}</h3>
            <p className="text-gray-700 dark:text-gray-300 mb-4">{t('data.downloadHistoricalSensorReadings')}</p>
            <div className="space-y-2 mb-4">
              <div className="flex items-center gap-2">
                <label htmlFor="start-date" className="text-gray-700 dark:text-gray-300 w-16">{t('data.from')}:</label>
                <input
                  type="date"
                  id="start-date"
                  value={startDate}
                  onChange={(e) => setStartDate(e.target.value)}
                  className="px-4 py-2 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                />
              </div>
              <div className="flex items-center gap-2">
                <label htmlFor="end-date" className="text-gray-700 dark:text-gray-300 w-16">{t('data.to')}:</label>
                <input
                  type="date"
                  id="end-date"
                  value={endDate}
                  onChange={(e) => setEndDate(e.target.value)}
                  className="px-4 py-2 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                />
              </div>
            </div>
            <button 
              onClick={downloadSensorData} 
              className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
            >
              {t('data.downloadSensorData')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default DataPage; 