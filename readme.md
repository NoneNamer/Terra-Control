Terrarium Controller <br />

Main Functions <br />
1. UV, LED and heating control on a configurable Timer <br />
2. Overheat Protection <br />
3. Temperature and humidity logging <br />
4. UV level Monitoring <br />
5. Webinterface for configuration and monitoring <br />

Hardware <br />

Raspberry Pi4 <br />
DS18B20 temperature sensor (x2) <br />
DHT22 Sensor / SHT31-D temperature and humidity Sensor <br />
GUVA-S12SD UV sensor <br />
WS2812B 4m 60p/m 12v min ip64                  # Need better LEDs <br />
SSD1306 OLED Display / SH1106 OLED Display <br />
Mean Well RD-65a <br />
Phillips HID- PV C 70 <br />

Data Structure <br />
├── main 					# Main entry point <br />
├── config 					# Configuration file for base schedules and thresholds <br />
├── requirements.txt 		# Dependencies <br />
├── /modules 				# Custom hardware interface modules <br />
│   ├── mod.rs 				# Modules entry point <br />
│   ├── getData 			# Read all Sensors <br />
│   ├── lightControl 		# Control of UV1, UV2 and heatspot <br />
│   ├── ledStrip 			# Control of LED lighting <br />
│   ├── display 			# Display <br />
│   ├── web 				# Webserver handling user input and serve data <br />
│   └── cam 				# Camera handling <br />
├── /logs 					# Logs <br />
│   ├── terra-sys.log 		# Log <br />
│   └── terra-temp.log 		# Seperate logfile for overheat <br />
└── /static					# Web assets <br />
    ├── index.html <br />
    ├── schedule.html <br />
    ├──	data.html <br />
    └── cam.html <br />