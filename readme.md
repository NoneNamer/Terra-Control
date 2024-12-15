Terrarium Controller <br />
WIP <br />

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
├── main.rs             # Main entry point <br />
├── config.toml         # Configuration file <br />
├── Cargo.toml          # Project dependencies / config <br />
├── /modules            # Modules <br />
│   ├── mod.rs          # Entry point for modules <br />
│   ├── config.rs       # Handles loading from config.toml <br />
│   ├── getData.rs      # Sensor reading logic <br />
│   ├── lightControl.rs # UV and heatspot control <br />
│   ├── ledStrip.rs     # LED lighting control <br />
│   ├── display.rs      # Display control <br />
│   ├── web.rs          # Web server logic <br />
│   └── cam.rs          # Camera handling logic <br />
├── /logs               # Log files directory <br />
│   ├── terra-sys.log <br />
│   └── terra-temp.log <br />
└── /static             # Web assets <br />
    ├── index.html <br />
    ├── schedule.html <br />
    ├── data.html <br />
    └── cam.html <br />