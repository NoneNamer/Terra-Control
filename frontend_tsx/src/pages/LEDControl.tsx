import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';
import '../styles/LEDControl.css';

interface LEDStatus {
  power: boolean;
  use_natural: boolean;
  r: number;
  g: number;
  b: number;
  ww: number;
  cw: number;
  season_weight: number;
}

interface NaturalLightPresets {
  morning_r: number;
  morning_g: number;
  morning_b: number;
  morning_ww: number;
  morning_cw: number;
  noon_r: number;
  noon_g: number;
  noon_b: number;
  noon_ww: number;
  noon_cw: number;
  evening_r: number;
  evening_g: number;
  evening_b: number;
  evening_ww: number;
  evening_cw: number;
}

const LEDControl: React.FC = () => {
  const { t } = useTranslation();
  const [ledStatus, setLedStatus] = useState<LEDStatus>({
    power: false,
    use_natural: true,
    r: 0,
    g: 0,
    b: 0,
    ww: 0,
    cw: 0,
    season_weight: 0.3
  });

  const [presets, setPresets] = useState<NaturalLightPresets>({
    morning_r: 255,
    morning_g: 180,
    morning_b: 100,
    morning_ww: 200,
    morning_cw: 50,
    noon_r: 255,
    noon_g: 240,
    noon_b: 220,
    noon_ww: 50,
    noon_cw: 255,
    evening_r: 255,
    evening_g: 140,
    evening_b: 50,
    evening_ww: 255,
    evening_cw: 0
  });

  const [timeValue, setTimeValue] = useState(12);

  useEffect(() => {
    fetchLEDStatus();
    fetchNaturalLightPresets();
    const interval = setInterval(fetchLEDStatus, 30000);
    return () => clearInterval(interval);
  }, []);

  const fetchLEDStatus = async () => {
    try {
      const response = await fetch('/api/led/status');
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      const data = await response.json();
      setLedStatus(data);
    } catch (error) {
      console.error('Error fetching LED status:', error);
    }
  };

  const fetchNaturalLightPresets = async () => {
    try {
      const response = await fetch('/api/led/presets');
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      const data = await response.json();
      setPresets(data);
    } catch (error) {
      console.error('Error fetching natural light presets:', error);
    }
  };

  const setLEDPower = async (power: boolean) => {
    try {
      const response = await fetch('/api/led/power', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ power }),
      });
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      setLedStatus(prev => ({ ...prev, power }));
    } catch (error) {
      console.error('Error setting LED power:', error);
    }
  };

  const setLEDColor = async (r: number, g: number, b: number, ww: number, cw: number) => {
    try {
      const response = await fetch('/api/led/color', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ r, g, b, ww, cw }),
      });
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      setLedStatus(prev => ({ ...prev, r, g, b, ww, cw }));
    } catch (error) {
      console.error('Error setting LED color:', error);
    }
  };

  const setNaturalLightSettings = async (override_settings: boolean, season_weight: number) => {
    try {
      const response = await fetch('/api/led/natural', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ override_settings, season_weight }),
      });
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      setLedStatus(prev => ({ ...prev, use_natural: !override_settings, season_weight }));
    } catch (error) {
      console.error('Error setting natural light settings:', error);
    }
  };

  const saveNaturalLightPresets = async () => {
    try {
      const response = await fetch('/api/led/presets', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(presets),
      });
      if (!response.ok) throw new Error(`HTTP error! Status: ${response.status}`);
      alert(t('led.presetsSaved'));
    } catch (error) {
      console.error('Error saving natural light presets:', error);
      alert(t('led.errorSavingPresets') + ': ' + error);
    }
  };

  const updateLightPreview = (r: number, g: number, b: number, ww: number, cw: number) => {
    const finalR = Math.min(255, r + ww * 0.8);
    const finalG = Math.min(255, g + ww * 0.6 + cw * 0.5);
    const finalB = Math.min(255, b + cw * 0.9);
    return `rgb(${finalR}, ${finalG}, ${finalB})`;
  };

  const interpolate = (start: number, end: number, factor: number) => {
    return Math.round(start + (end - start) * factor);
  };

  const updateCycleLightPreview = () => {
    const hours = Math.floor(timeValue);
    const minutes = (timeValue - hours) * 60;
    const timeString = `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;

    const morningTime = 7;
    const noonTime = 12;
    const eveningTime = 19;

    let r, g, b, ww, cw;

    if (timeValue >= morningTime && timeValue < noonTime) {
      const factor = (timeValue - morningTime) / (noonTime - morningTime);
      r = interpolate(presets.morning_r, presets.noon_r, factor);
      g = interpolate(presets.morning_g, presets.noon_g, factor);
      b = interpolate(presets.morning_b, presets.noon_b, factor);
      ww = interpolate(presets.morning_ww, presets.noon_ww, factor);
      cw = interpolate(presets.morning_cw, presets.noon_cw, factor);
    } else if (timeValue >= noonTime && timeValue < eveningTime) {
      const factor = (timeValue - noonTime) / (eveningTime - noonTime);
      r = interpolate(presets.noon_r, presets.evening_r, factor);
      g = interpolate(presets.noon_g, presets.evening_g, factor);
      b = interpolate(presets.noon_b, presets.evening_b, factor);
      ww = interpolate(presets.noon_ww, presets.evening_ww, factor);
      cw = interpolate(presets.noon_cw, presets.evening_cw, factor);
    } else {
      r = presets.evening_r;
      g = presets.evening_g;
      b = presets.evening_b;
      ww = presets.evening_ww;
      cw = presets.evening_cw;
    }

    return updateLightPreview(r, g, b, ww, cw);
  };

  return (
    <div className="container mx-auto p-4 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
      <h1 className="text-2xl font-bold mb-4 text-gray-900 dark:text-white">{t('led.title')}</h1>
      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <div className="mb-8">
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('led.manualOverride')}</h2>
        <p className="text-gray-700 dark:text-gray-300 mb-4">{t('led.overrideDescription')}</p>
        <form onSubmit={(e) => {
          e.preventDefault();
          setNaturalLightSettings(!ledStatus.use_natural, ledStatus.season_weight);
          setLEDColor(ledStatus.r, ledStatus.g, ledStatus.b, ledStatus.ww, ledStatus.cw);
        }}>
          <div className="bg-white dark:bg-gray-700 p-4 rounded-lg mb-4">
            <div className="grid grid-cols-2 gap-4 mb-4">
              <div className="flex items-center gap-2">
                <label htmlFor="override" className="text-gray-700 dark:text-gray-300">{t('led.overrideNaturalLight')}:</label>
                <input
                  type="checkbox"
                  id="override"
                  checked={!ledStatus.use_natural}
                  onChange={(e) => setNaturalLightSettings(!e.target.checked, ledStatus.season_weight)}
                  className="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                />
              </div>
              
              <div className="flex items-center gap-2">
                <label htmlFor="ledPower" className="text-gray-700 dark:text-gray-300">{t('led.ledPower')}:</label>
                <input
                  type="checkbox"
                  id="ledPower"
                  checked={ledStatus.power}
                  onChange={(e) => setLEDPower(e.target.checked)}
                  className="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                />
              </div>
            </div>

            <div className="space-y-4">
              <div className="flex items-center gap-4">
                <label htmlFor="red" className="text-gray-700 dark:text-gray-300 w-16">{t('led.red')}:</label>
                <input
                  type="range"
                  id="red"
                  min="0"
                  max="255"
                  value={ledStatus.r}
                  onChange={(e) => setLedStatus(prev => ({ ...prev, r: parseInt(e.target.value) }))}
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
                />
                <span className="text-gray-700 dark:text-gray-300 w-12">{ledStatus.r}</span>
              </div>

              <div className="flex items-center gap-4">
                <label htmlFor="green" className="text-gray-700 dark:text-gray-300 w-16">{t('led.green')}:</label>
                <input
                  type="range"
                  id="green"
                  min="0"
                  max="255"
                  value={ledStatus.g}
                  onChange={(e) => setLedStatus(prev => ({ ...prev, g: parseInt(e.target.value) }))}
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
                />
                <span className="text-gray-700 dark:text-gray-300 w-12">{ledStatus.g}</span>
              </div>

              <div className="flex items-center gap-4">
                <label htmlFor="blue" className="text-gray-700 dark:text-gray-300 w-16">{t('led.blue')}:</label>
                <input
                  type="range"
                  id="blue"
                  min="0"
                  max="255"
                  value={ledStatus.b}
                  onChange={(e) => setLedStatus(prev => ({ ...prev, b: parseInt(e.target.value) }))}
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
                />
                <span className="text-gray-700 dark:text-gray-300 w-12">{ledStatus.b}</span>
              </div>

              <div className="flex items-center gap-4">
                <label htmlFor="ww" className="text-gray-700 dark:text-gray-300 w-16">{t('led.warmWhite')}:</label>
                <input
                  type="range"
                  id="ww"
                  min="0"
                  max="255"
                  value={ledStatus.ww}
                  onChange={(e) => setLedStatus(prev => ({ ...prev, ww: parseInt(e.target.value) }))}
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
                />
                <span className="text-gray-700 dark:text-gray-300 w-12">{ledStatus.ww}</span>
              </div>

              <div className="flex items-center gap-4">
                <label htmlFor="cw" className="text-gray-700 dark:text-gray-300 w-16">{t('led.coldWhite')}:</label>
                <input
                  type="range"
                  id="cw"
                  min="0"
                  max="255"
                  value={ledStatus.cw}
                  onChange={(e) => setLedStatus(prev => ({ ...prev, cw: parseInt(e.target.value) }))}
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
                />
                <span className="text-gray-700 dark:text-gray-300 w-12">{ledStatus.cw}</span>
              </div>
            </div>

            <div className="mt-4">
              <button
                type="submit"
                className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
              >
                {t('led.applySettings')}
              </button>
            </div>
          </div>
        </form>
      </div>

      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <div className="mb-8">
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('led.naturalLightCycle')}</h2>
        <div className="bg-white dark:bg-gray-700 p-4 rounded-lg">
          <div className="mb-4">
            <label htmlFor="timeValue" className="block text-gray-700 dark:text-gray-300 mb-2">{t('led.timeOfDay')}:</label>
            <input
              type="range"
              id="timeValue"
              min="0"
              max="24"
              step="0.25"
              value={timeValue}
              onChange={(e) => setTimeValue(parseFloat(e.target.value))}
              className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
            />
            <div className="text-gray-700 dark:text-gray-300 mt-2">
              {Math.floor(timeValue).toString().padStart(2, '0')}:{((timeValue % 1) * 60).toString().padStart(2, '0')}
            </div>
          </div>

          <div className="mb-4">
            <label htmlFor="seasonWeight" className="block text-gray-700 dark:text-gray-300 mb-2">{t('led.seasonWeight')}:</label>
            <input
              type="range"
              id="seasonWeight"
              min="0"
              max="1"
              step="0.1"
              value={ledStatus.season_weight}
              onChange={(e) => setNaturalLightSettings(!ledStatus.use_natural, parseFloat(e.target.value))}
              className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
            />
            <div className="text-gray-700 dark:text-gray-300 mt-2">
              {ledStatus.season_weight.toFixed(1)}
            </div>
          </div>

          <div className="mb-4">
            <h3 className="text-lg font-semibold mb-2 text-gray-800 dark:text-gray-100">{t('led.lightPreview')}</h3>
            <div
              className="w-full h-32 rounded-lg border-2 border-gray-300 dark:border-gray-600 shadow-md"
              style={{ backgroundColor: updateCycleLightPreview() }}
            />
          </div>
        </div>
      </div>

      <hr className="my-4 border-gray-300 dark:border-gray-600" />

      <div>
        <h2 className="text-xl font-semibold mb-4 text-gray-800 dark:text-gray-100">{t('led.presetManagement')}</h2>
        <div className="bg-white dark:bg-gray-700 p-4 rounded-lg">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <h3 className="text-lg font-semibold mb-2 text-gray-800 dark:text-gray-100">{t('led.morning')}</h3>
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.red')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.morning_r}
                    onChange={(e) => setPresets(prev => ({ ...prev, morning_r: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.green')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.morning_g}
                    onChange={(e) => setPresets(prev => ({ ...prev, morning_g: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.blue')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.morning_b}
                    onChange={(e) => setPresets(prev => ({ ...prev, morning_b: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.warmW')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.morning_ww}
                    onChange={(e) => setPresets(prev => ({ ...prev, morning_ww: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.coldW')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.morning_cw}
                    onChange={(e) => setPresets(prev => ({ ...prev, morning_cw: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
              </div>
            </div>

            <div>
              <h3 className="text-lg font-semibold mb-2 text-gray-800 dark:text-gray-100">{t('led.noon')}</h3>
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.red')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.noon_r}
                    onChange={(e) => setPresets(prev => ({ ...prev, noon_r: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.green')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.noon_g}
                    onChange={(e) => setPresets(prev => ({ ...prev, noon_g: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.blue')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.noon_b}
                    onChange={(e) => setPresets(prev => ({ ...prev, noon_b: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.warmW')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.noon_ww}
                    onChange={(e) => setPresets(prev => ({ ...prev, noon_ww: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.coldW')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.noon_cw}
                    onChange={(e) => setPresets(prev => ({ ...prev, noon_cw: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
              </div>
            </div>

            <div>
              <h3 className="text-lg font-semibold mb-2 text-gray-800 dark:text-gray-100">{t('led.evening')}</h3>
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.red')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.evening_r}
                    onChange={(e) => setPresets(prev => ({ ...prev, evening_r: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.green')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.evening_g}
                    onChange={(e) => setPresets(prev => ({ ...prev, evening_g: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.blue')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.evening_b}
                    onChange={(e) => setPresets(prev => ({ ...prev, evening_b: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.warmW')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.evening_ww}
                    onChange={(e) => setPresets(prev => ({ ...prev, evening_ww: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label className="text-gray-700 dark:text-gray-300 w-16">{t('led.coldW')}:</label>
                  <input
                    type="number"
                    min="0"
                    max="255"
                    value={presets.evening_cw}
                    onChange={(e) => setPresets(prev => ({ ...prev, evening_cw: parseInt(e.target.value) }))}
                    className="px-2 py-1 bg-white dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded border border-gray-300 dark:border-gray-500"
                  />
                </div>
              </div>
            </div>
          </div>

          <div className="mt-4">
            <button
              onClick={saveNaturalLightPresets}
              className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
            >
              {t('led.savePresets')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default LEDControl; 