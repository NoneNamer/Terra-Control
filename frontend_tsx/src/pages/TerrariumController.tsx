import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Line } from 'react-chartjs-2';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend
} from 'chart.js';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend
);

interface SensorData {
  baskingTemp: number;
  controlTemp: number;
  coolZoneTemp: number;
  humidity: number;
  uv1: number;
  uv2: number;
  uv1_on: boolean;
  uv2_on: boolean;
}

interface GraphDataPoint {
  time: string;
  temperature: number;
  coolZoneTemp: number;
  humidity: number;
  uv1: number;
  uv2: number;
}

const TerrariumController = () => {
  const { t } = useTranslation();
  const [sensorData, setSensorData] = useState<SensorData | null>(null);
  const [todayData, setTodayData] = useState<GraphDataPoint[]>([]);
  const [yesterdayData, setYesterdayData] = useState<GraphDataPoint[]>([]);

  const fetchCurrentValues = async () => {
    try {
      const response = await fetch('/api/values');
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      const data = await response.json();
      setSensorData(data);
    } catch (error) {
      console.error('Error fetching current values:', error);
    }
  };

  const fetchGraphData = async (endpoint: string, setData: (data: GraphDataPoint[]) => void) => {
    try {
      const response = await fetch(`/api/graph/${endpoint}`);
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      const data = await response.json();
      setData(data);
    } catch (error) {
      console.error(`Error fetching ${endpoint} graph data:`, error);
    }
  };

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: true,
    scales: {
      y: {
        beginAtZero: true,
        max: 70
      }
    }
  };

  const createChartData = (data: GraphDataPoint[], title: string) => ({
    labels: data.map(d => d.time),
    datasets: [
      {
        label: t('terrarium.baskingTemp1'),
        data: data.map(d => d.temperature),
        borderColor: 'rgb(255, 99, 132)',
        fill: false
      },
      {
        label: t('terrarium.coolZoneTemp'),
        data: data.map(d => d.coolZoneTemp),
        borderColor: 'rgb(54, 162, 235)',
        fill: false
      },
      {
        label: t('terrarium.humidity'),
        data: data.map(d => d.humidity),
        borderColor: 'rgb(75, 192, 192)',
        fill: false
      },
      {
        label: 'UV1',
        data: data.map(d => d.uv1),
        borderColor: 'rgb(255, 159, 64)',
        fill: false
      },
      {
        label: 'UV2',
        data: data.map(d => d.uv2),
        borderColor: 'rgb(153, 102, 255)',
        fill: false
      }
    ]
  });

  useEffect(() => {
    fetchCurrentValues();
    fetchGraphData('today', setTodayData);
    fetchGraphData('yesterday', setYesterdayData);

    const interval = setInterval(() => {
      fetchCurrentValues();
      fetchGraphData('today', setTodayData);
    }, 60000);

    return () => clearInterval(interval);
  }, []);

  return (
    <div className="container mx-auto p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
      <h1 className="text-2xl font-bold mb-4 text-gray-900 dark:text-white">{t('terrarium.title')}</h1>
      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('terrarium.currentReadings')}</h2>
      <div className="grid grid-cols-2 gap-4 mb-4 text-gray-700 dark:text-gray-300">
        <p>{t('terrarium.baskingTemp1')}: <span className="font-semibold">{sensorData?.baskingTemp.toFixed(1) ?? '--'}</span>°C</p>
        <p>{t('terrarium.baskingTemp2')}: <span className="font-semibold">{sensorData?.controlTemp.toFixed(1) ?? '--'}</span>°C</p>
        <p>{t('terrarium.coolZoneTemp')}: <span className="font-semibold">{sensorData?.coolZoneTemp.toFixed(1) ?? '--'}</span>°C</p>
        <p>{t('terrarium.humidity')}: <span className="font-semibold">{sensorData?.humidity.toFixed(1) ?? '--'}</span>%</p>
      </div>
      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-8">
        <div className="h-64 bg-white dark:bg-gray-700 p-4 rounded-lg">
          <Line options={chartOptions} data={createChartData(todayData, t('terrarium.today'))} />
        </div>
        <div className="h-64 bg-white dark:bg-gray-700 p-4 rounded-lg">
          <Line options={chartOptions} data={createChartData(yesterdayData, t('terrarium.yesterday'))} />
        </div>
      </div>

      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('terrarium.uvStatus')}</h2>
      <div className="grid grid-cols-2 gap-4 text-gray-700 dark:text-gray-300">
        <p>UV1: <span className="font-semibold">{sensorData?.uv1.toFixed(1) ?? '--'}</span> UVI <span>{sensorData?.uv1_on ? '✅' : '❌'}</span></p>
        <p>UV2: <span className="font-semibold">{sensorData?.uv2.toFixed(1) ?? '--'}</span> UVI <span>{sensorData?.uv2_on ? '✅' : '❌'}</span></p>
      </div>
    </div>
  );
};

export default TerrariumController; 