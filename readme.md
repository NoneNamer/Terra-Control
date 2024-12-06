Terrarium Controller

Main Functions
1. UV, LED and heating control on a configurable Timer
2. Overheat Protection
3. Temperature and humidity logging
4. UV level Monitoring
5. Webinterface for configuration and monitoring

Hardware

Raspberry Pi4
DS18B20 temperature sensor (x2)
DHT22 Sensor / SHT31-D temperature and humidity Sensor
GUVA-S12SD UV sensor
WS2812B 4m 60p/m 12v min ip64                  # Need better LEDs
SSD1306 OLED Display / SH1106 OLED Display
Mean Well RD-65a
Phillips HID- PV C 70

Data Structure
├── main                  # Main entry point
├── config                # Configuration file for schedules and thresholds
├── requirements.txt      # Dependencies
├── /modules              # Custom hardware interface modules
│   ├── get_data          # Read all Sensors
│   ├── light_control     # Control of UV1, UV2 and heatspot
│   ├── led_strip         # Control of LED lighting
│   ├── oled_display      # Display
│   └── web_server        # Webserver handling user input and serve data
├── /logs                 # Logs
│   ├── terra-sys.log     # Log
│   └── terra-temp.log    # Seperate logfile for overheat
└── /static               # Web assets (HTML, CSS, JS)
