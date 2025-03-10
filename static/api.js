const API_BASE_URL = '/api';

class TerrariumAPI {
    static async getCurrentReadings() {
        const response = await fetch(`${API_BASE_URL}/current`);
        return response.json();
    }

    static async getDailyData(date) {
        const response = await fetch(`${API_BASE_URL}/data/${date}`);
        return response.json();
    }

    static async getAllSchedules() {
        const response = await fetch(`${API_BASE_URL}/schedule`);
        return response.json();
    }

    static async updateSchedule(week, settings) {
        const response = await fetch(`${API_BASE_URL}/schedule/${week}`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(settings),
        });
        return response.ok;
    }

    static async updateRGBOverride(settings) {
        const response = await fetch(`${API_BASE_URL}/override`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(settings),
        });
        return response.ok;
    }
    
    // New methods for log and data functionality
    static async getLogEntries(filter = 'all', limit = 50) {
        const response = await fetch(`${API_BASE_URL}/logs?filter=${filter}&limit=${limit}`);
        if (!response.ok) {
            throw new Error(`Failed to fetch logs: ${response.statusText}`);
        }
        return response.json();
    }
    
    static async downloadLogs() {
        const response = await fetch(`${API_BASE_URL}/logs/download`);
        if (!response.ok) {
            throw new Error(`Failed to download logs: ${response.statusText}`);
        }
        
        const blob = await response.blob();
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.style.display = 'none';
        a.href = url;
        
        // Get filename from Content-Disposition header or use default
        const contentDisposition = response.headers.get('Content-Disposition');
        let filename = 'terrarium_logs.zip';
        if (contentDisposition) {
            const filenameMatch = contentDisposition.match(/filename="(.+)"/);
            if (filenameMatch) {
                filename = filenameMatch[1];
            }
        }
        
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        window.URL.revokeObjectURL(url);
        document.body.removeChild(a);
    }
    
    static async downloadSensorData(startDate, endDate) {
        const response = await fetch(`${API_BASE_URL}/data/download?start=${startDate}&end=${endDate}`);
        if (!response.ok) {
            throw new Error(`Failed to download sensor data: ${response.statusText}`);
        }
        
        const blob = await response.blob();
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.style.display = 'none';
        a.href = url;
        
        // Get filename from Content-Disposition header or use default
        const contentDisposition = response.headers.get('Content-Disposition');
        let filename = `terrarium_data_${startDate}_to_${endDate}.csv`;
        if (contentDisposition) {
            const filenameMatch = contentDisposition.match(/filename="(.+)"/);
            if (filenameMatch) {
                filename = filenameMatch[1];
            }
        }
        
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        window.URL.revokeObjectURL(url);
        document.body.removeChild(a);
    }
}

// index.js
async function updateCurrentReadings() {
    try {
        const data = await TerrariumAPI.getCurrentReadings();
        document.getElementById('baskingTemp').textContent = data.baskingTemp.toFixed(1);
        document.getElementById('controlTemp').textContent = data.controlTemp.toFixed(1);
        document.getElementById('coolZoneTemp').textContent = data.coolZoneTemp.toFixed(1);
        document.getElementById('humidity').textContent = data.humidity.toFixed(1);
        
        // Update UV status with proper indicators
        document.getElementById('UV1').textContent = `${data.uv1.value} UVI ${data.uv1.ok ? '✅' : '❌'}`;
        document.getElementById('UV2').textContent = `${data.uv2.value} UVI ${data.uv2.ok ? '✅' : '❌'}`;
    } catch (error) {
        console.error('Failed to update readings:', error);
    }
}

// Initialize charts
async function initializeCharts() {
    try {
        const todayData = await TerrariumAPI.getDailyData(new Date().toISOString().split('T')[0]);
        const yesterdayData = await TerrariumAPI.getDailyData(
            new Date(Date.now() - 86400000).toISOString().split('T')[0]
        );

        createChart(document.getElementById('todayGraph').getContext('2d'), 
                   todayData, 
                   `Data for ${formatDate(new Date())}`);
        
        createChart(document.getElementById('yesterdayGraph').getContext('2d'), 
                   yesterdayData, 
                   `Data for ${formatDate(new Date(Date.now() - 86400000))}`);
    } catch (error) {
        console.error('Failed to initialize charts:', error);
    }
}

// Handle RGB override form
document.getElementById('rgbForm').addEventListener('submit', async (e) => {
    e.preventDefault();
    const formData = new FormData(e.target);
    
    try {
        await TerrariumAPI.updateRGBOverride({
            override: formData.get('override') === 'true',
            red: parseInt(formData.get('red')),
            green: parseInt(formData.get('green')),
            blue: parseInt(formData.get('blue')),
            warmWhite: parseInt(formData.get('wwhite')),
            coolWhite: parseInt(formData.get('cwhite')),
        });
        alert('RGB override values updated successfully');
    } catch (error) {
        alert('Failed to update RGB settings');
        console.error(error);
    }
});

// schedule.js
async function loadScheduleData() {
    try {
        const schedules = await TerrariumAPI.getAllSchedules();
        schedules.forEach(schedule => {
            updateWeekForm(schedule.week, schedule);
        });
    } catch (error) {
        console.error('Failed to load schedule data:', error);
    }
}

function updateWeekForm(week, data) {
    const weekPrefix = `week${week}`;
    for (const [key, value] of Object.entries(data)) {
        const element = document.getElementById(`${key}${week}`);
        if (element) {
            element.value = value;
        }
    }
}

// schedule form submission
document.getElementById('weeklySettingsForm').addEventListener('submit', async (e) => {
    e.preventDefault();
    const updates = [];
    
    for (let week = 1; week <= 52; week++) {
        updates.push({
            week,
            uv1_start: document.getElementById(`uv1Start${week}`).value,
            uv1_end: document.getElementById(`uv1End${week}`).value,
            uv2_start: document.getElementById(`uv2Start${week}`).value,
            uv2_end: document.getElementById(`uv2End${week}`).value,
            heat_start: document.getElementById(`heatStart${week}`).value,
            heat_end: document.getElementById(`heatEnd${week}`).value,
            led: {
                red: parseInt(document.getElementById(`red${week}`).value),
                green: parseInt(document.getElementById(`green${week}`).value),
                blue: parseInt(document.getElementById(`blue${week}`).value),
                cool_white: parseInt(document.getElementById(`cw${week}`).value),
                warm_white: parseInt(document.getElementById(`ww${week}`).value),
            }
        });
    }
    
    try {
        for (const update of updates) {
            await TerrariumAPI.updateSchedule(update.week, update);
        }
        alert('Schedule updated successfully');
    } catch (error) {
        alert('Failed to update schedule');
        console.error(error);
    }
});

// periodic updates for the dashboard
if (window.location.pathname.endsWith('index.html')) {
    updateCurrentReadings();
    initializeCharts();
    setInterval(updateCurrentReadings, 30000); // Update every 30 seconds
}

// Load schedule data on demand
if (window.location.pathname.endsWith('schedule.html')) {
    loadScheduleData();
}