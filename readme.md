# Terrarium Controller WIP minimal version

## Overview

The system manages lighting, heating, and environmental conditions while providing real-time monitoring through a web interface. It's designed with reliability and customization.

## Features

- **Environmental Control**
  - UV, LED, and heating management on configurable timers
  - Smart day/night cycle lighting simulation
  - Overheat protection with automatic shutdown

- **Monitoring**
  - Temperature and humidity tracking from multiple sensors
  - Real-time data logging
  
- **User Interface**
  - Web interface for configuration and monitoring
  - Mobile-friendly responsive design

## Hardware Requirements

- Raspberry Pi 3a+
- Temperature Sensors:
  - DS18B20 temperature sensors (×2)
  - DHT22 temperature and humidity sensor
- WS2805 LED strip for ambient lighting
- Power Management:
  - Mean Well RD-65b power supply
  - Phillips HID-PV C 70 ballast
- Relay board for controlling heating elements, UV lighting and LED power supply


## Project Structure

```
├── main.rs             # Main entry point
├── config.toml         # Configuration file
├── Cargo.toml          # Project dependencies / config
├── /modules            # Modules
│   ├── mod.rs          # Entry point for modules
│   ├── models.rs       # Data models
│   ├── config.rs       # Handles loading from config.toml
│   ├── gpio.rs         # GPIO out module
│   ├── getData.rs      # Sensor reading logic
│   ├── storage.rs      # SQLite DB schedule handling
│   ├── lightControl.rs # UV and heatspot control
│   ├── ledStrip.rs     # LED lighting control
│   ├── logs.rs         # Log managemant module
│   └── web.rs          # Web server logic
├── /logs               # Log files directory
│   ├── terra-sys.log   # System logs
│   └── terra-temp.log  # Temperature logs
├── /static             # Web assets
│   ├── styles.css      # CSS styles
│   ├── index.html      # Main dashboard
│   ├── schedule.html   # Schedule configuration
│   ├── data.html       # Data visualization
│   ├── led.html        # LED controll
│   └── cam.html        # Camera stream page
└── /lib                # External libraries
```

## Usage

1. **Configuration**
   - Edit `config.toml` to adjust hardware settings, scheduling defaults, and system parameters
   - Web interface provides most common configuration options

2. **Web Interface**
   - Access the web interface at `http://your-raspberry-pi-ip:80`
   - Configure schedules, view current readings, and access the camera stream

3. **Monitoring**
   - Temperature, humidity, and UV data are logged to the database
   - View historical data through the web interface charts
   - System logs capture events and potential issues

## Development

This project is built with:
- Rust programming language
- Tokio for async runtime
- SQLite for data storage
- Embedded hardware interfaces for sensors and controls
- Axum for the web server

## License



## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

