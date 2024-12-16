Terrarium Controller <br />
WIP <br />

Main Functions <br />
1. UV, LED and heating control on a configurable timer <br />
2. Overheat protection <br />
3. Temperature and humidity logging <br />
4. UV level monitoring <br />
5. Webinterface for configuration and monitoring <br />
6. Camera stream to webpage <br />

Hardware <br />

Raspberry Pi4 <br />
DS18B20 temperature sensor (x2) <br />
DHT22 Sensor / SHT31-D temperature and humidity Sensor <br />
VEML6075 UV sensor <br />
WS2812B 4m 60p/m 12v min ip64                  # Need better RGBWW LEDs <br />
SSD1306 OLED Display / SH1106 OLED Display <br />
Mean Well RD-65a/b                             # Model dependent on LED voltage <br />
Phillips HID- PV C 70 <br />

Data Structure <br />
├── main.rs             # Main entry point <br />
├── config.toml         # Configuration file <br />
├── Cargo.toml          # Project dependencies / config <br />
├── /modules            # Modules <br />
│   ├── mod.rs          # Entry point for modules <br />
│   ├── config.rs       # Handles loading from config.toml <br />
│   ├── getData.rs      # Sensor reading logic <br />
│   ├── schedule.rs     # sqlite db schedule handling <br />
│   ├── lightControl.rs # UV and heatspot control <br />
│   ├── led.rs          # LED lighting control <br />
│   ├── display.rs      # Display control <br />
│   ├── web.rs          # Web server logic <br />
│   └── cam.rs          # Camera handling logic <br />
├── /logs               # Log files directory <br />
│   ├── terra-sys.log <br />
│   └── terra-temp.log <br />
├── /static             # Web assets <br />
│   ├── styles.css <br />
│   ├── index.html <br />
│   ├── schedule.html <br />
│   ├── data.html <br />
│   └── cam.html <br />
└── /lib                # Lib <br />