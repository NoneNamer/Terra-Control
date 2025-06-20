import { useState } from 'react';

interface WeekSettings {
  uv1Start: string;
  uv1End: string;
  uv2Start: string;
  uv2End: string;
  heatStart: string;
  heatEnd: string;
  red: number;
  green: number;
  blue: number;
  cw: number;
  ww: number;
}

interface WeekContainerProps {
  weekNumber: number;
  settings: WeekSettings;
  onSettingsChange: (weekNumber: number, settings: WeekSettings) => void;
}

const WeekContainer: React.FC<WeekContainerProps> = ({ weekNumber, settings, onSettingsChange }) => {
  const handleChange = (field: keyof WeekSettings, value: string | number) => {
    onSettingsChange(weekNumber, { ...settings, [field]: value });
  };

  return (
    <div className="bg-white dark:bg-gray-700 p-4 rounded-lg shadow-sm mb-4">
      <h3 className="text-lg font-semibold mb-4 text-gray-800 dark:text-gray-100">KW {weekNumber}</h3>
      <div className="grid grid-cols-2 gap-4">
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">UV1 Start:</label>
          <input
            type="time"
            value={settings.uv1Start}
            onChange={(e) => handleChange('uv1Start', e.target.value)}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">UV1 Ende:</label>
          <input
            type="time"
            value={settings.uv1End}
            onChange={(e) => handleChange('uv1End', e.target.value)}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">UV2 Start:</label>
          <input
            type="time"
            value={settings.uv2Start}
            onChange={(e) => handleChange('uv2Start', e.target.value)}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">UV2 Ende:</label>
          <input
            type="time"
            value={settings.uv2End}
            onChange={(e) => handleChange('uv2End', e.target.value)}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Heizung Start:</label>
          <input
            type="time"
            value={settings.heatStart}
            onChange={(e) => handleChange('heatStart', e.target.value)}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Heizung Ende:</label>
          <input
            type="time"
            value={settings.heatEnd}
            onChange={(e) => handleChange('heatEnd', e.target.value)}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Rot:</label>
          <input
            type="number"
            min="0"
            max="255"
            value={settings.red}
            onChange={(e) => handleChange('red', parseInt(e.target.value))}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Grün:</label>
          <input
            type="number"
            min="0"
            max="255"
            value={settings.green}
            onChange={(e) => handleChange('green', parseInt(e.target.value))}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Blau:</label>
          <input
            type="number"
            min="0"
            max="255"
            value={settings.blue}
            onChange={(e) => handleChange('blue', parseInt(e.target.value))}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Kaltweiß:</label>
          <input
            type="number"
            min="0"
            max="255"
            value={settings.cw}
            onChange={(e) => handleChange('cw', parseInt(e.target.value))}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
        <div className="input-group">
          <label className="block text-sm font-medium mb-1 text-gray-700 dark:text-gray-300">Warmweiß:</label>
          <input
            type="number"
            min="0"
            max="255"
            value={settings.ww}
            onChange={(e) => handleChange('ww', parseInt(e.target.value))}
            className="w-full p-2 border rounded bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-500"
          />
        </div>
      </div>
    </div>
  );
};

const Schedule: React.FC = () => {
  const [weeklySettings, setWeeklySettings] = useState<Record<number, WeekSettings>>(() => {
    const initialSettings: Record<number, WeekSettings> = {};
    for (let week = 1; week <= 52; week++) {
      initialSettings[week] = {
        uv1Start: '06:00',
        uv1End: '18:00',
        uv2Start: '08:00',
        uv2End: '20:00',
        heatStart: '08:00',
        heatEnd: '20:00',
        red: 255,
        green: 200,
        blue: 100,
        cw: 150,
        ww: 200,
      };
    }
    return initialSettings;
  });

  const handleSettingsChange = (weekNumber: number, settings: WeekSettings) => {
    setWeeklySettings(prev => ({
      ...prev,
      [weekNumber]: settings
    }));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    // Hier können Sie die Daten an den Server senden
    console.log('Einstellungen gespeichert:', weeklySettings);
  };

  const renderQuarter = (startWeek: number, endWeek: number) => {
    return (
      <div className="mb-8">
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">Quartal {Math.ceil(startWeek / 13)}</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {Array.from({ length: endWeek - startWeek + 1 }, (_, i) => startWeek + i).map((week) => (
            <WeekContainer
              key={week}
              weekNumber={week}
              settings={weeklySettings[week]}
              onSettingsChange={handleSettingsChange}
            />
          ))}
        </div>
      </div>
    );
  };

  return (
    <div className="container mx-auto p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
      <h1 className="text-2xl font-bold mb-4 text-gray-900 dark:text-white">Wöchentliche Beleuchtungseinstellungen</h1>
      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <form onSubmit={handleSubmit}>
        {renderQuarter(1, 13)}
        {renderQuarter(14, 26)}
        {renderQuarter(27, 39)}
        {renderQuarter(40, 52)}
        <div className="mt-6">
          <button
            type="submit"
            className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
          >
            Einstellungen speichern
          </button>
        </div>
      </form>
    </div>
  );
};

export default Schedule; 