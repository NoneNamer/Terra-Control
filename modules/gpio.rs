use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use rppal::gpio::{Gpio, OutputPin};
use std::error::Error;
use std::thread;
use std::time::Duration;
use crate::modules::config::GpioConfig;

// WS2805 Constants (SPI Timing)
const T0H: u8 = 0b10000000; // ~312.5ns high
const T1H: u8 = 0b11000000; // ~625ns high
const RESET_TIME_US: u64 = 300; // >280µs reset time
const CHANNELS_PER_IC: usize = 5;  // Each WS2805 controls 5 LED channels
const BITS_PER_CHANNEL: usize = 8; // 8 bits per channel

/// Loads LED strip count from config
fn get_ic_count() -> usize {
    GpioConfig::load().ic_count.unwrap_or(16) // Default to 16 if not set
}

#[derive(Debug, Clone, Copy)]
pub struct RGBWW {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub ww: u8,
    pub cw: u8,
}

impl RGBWW {
    pub fn off() -> Self {
        Self { r: 0, g: 0, b: 0, ww: 0, cw: 0 }
    }

    pub fn from_str(s: &str) -> Result<Self, Box<dyn Error>> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 5 {
            return Err("LED values must be in format R,G,B,WW,CW".into());
        }
        Ok(Self {
            r: parts[0].parse()?,
            g: parts[1].parse()?,
            b: parts[2].parse()?,
            ww: parts[3].parse()?,
            cw: parts[4].parse()?,
        })
    }
}

/// Converts a byte to SPI bit timing format
fn convert_byte(byte: u8, buffer: &mut [u8]) {
    let mut byte = byte;
    for i in 0..8 {
        buffer[i] = if (byte & 0x80) != 0 { T1H } else { T0H };
        byte <<= 1;
    }
}

/// Controls an SPI-based LED strip
pub struct LEDStrip {
    spi: Spi,
    buffer: Vec<u8>,
    ic_count: usize,
}

impl LEDStrip {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let spi = Spi::new(
            Bus::Spi0,
            SlaveSelect::Ss0,
            3_200_000, // 3.2MHz for correct timing
            Mode::Mode0,
        )?;
        let ic_count = get_ic_count();
        let buffer = vec![0; ic_count * CHANNELS_PER_IC * BITS_PER_CHANNEL];
        Ok(Self { spi, buffer, ic_count })
    }

    pub fn set_all(&mut self, color: RGBWW) {
        for i in 0..self.ic_count {
            self.set_ic(i, color);
        }
    }

    pub fn set_ic(&mut self, index: usize, color: RGBWW) {
        if index >= self.ic_count {
            return;
        }
        let start = index * CHANNELS_PER_IC * BITS_PER_CHANNEL;
        convert_byte(color.g, &mut self.buffer[start..start + 8]);
        convert_byte(color.r, &mut self.buffer[start + 8..start + 16]);
        convert_byte(color.b, &mut self.buffer[start + 16..start + 24]);
        convert_byte(color.ww, &mut self.buffer[start + 24..start + 32]);
        convert_byte(color.cw, &mut self.buffer[start + 32..start + 40]);
    }

    pub fn show(&mut self) -> Result<(), Box<dyn Error>> {
        self.spi.write(&self.buffer)?;
        thread::sleep(Duration::from_micros(RESET_TIME_US));
        Ok(())
    }
}

/// Controls relays for UV, heat, and LED via GPIO 
pub struct RelayController { 
    uv1_relay: OutputPin,
    uv2_relay: OutputPin,
    heat_relay: OutputPin,
    led_relay: OutputPin,
} 

/// Defines the available relay types
#[derive(Debug, Clone, Copy)]
pub enum RelayType {
    UV1,
    UV2,
    Heat,
    LED,
}

impl RelayController { 
    pub fn new() -> Result<Self, Box<dyn Error>> { 
        let config = GpioConfig::load();
        let gpio = Gpio::new()?; 
        
        // Get pins from config
        let uv1_relay = gpio.get(config.uv_relay1)?.into_output();
        let uv2_relay = gpio.get(config.uv_relay2)?.into_output();
        let heat_relay = gpio.get(config.heat_relay)?.into_output();
        let led_relay = gpio.get(config.led_relay)?.into_output();
        
        Ok(Self { 
            uv1_relay,
            uv2_relay,
            heat_relay,
            led_relay,
        }) 
    } 

    /// Set a specific relay by type
    pub fn set_relay(&mut self, relay_type: RelayType, state: bool) {
        let pin = match relay_type {
            RelayType::UV1 => &mut self.uv1_relay,
            RelayType::UV2 => &mut self.uv2_relay,
            RelayType::Heat => &mut self.heat_relay,
            RelayType::LED => &mut self.led_relay,
        };
        
        pin.write(if state { rppal::gpio::Level::High } else { rppal::gpio::Level::Low });
    }
    
    /// Turn on a specific relay
    pub fn turn_on(&mut self, relay_type: RelayType) {
        self.set_relay(relay_type, true);
    }
    
    /// Turn off a specific relay
    pub fn turn_off(&mut self, relay_type: RelayType) {
        self.set_relay(relay_type, false);
    }
     
    /// Turn all relays off
    pub fn turn_all_off(&mut self) { 
        self.uv1_relay.set_low();
        self.uv2_relay.set_low();
        self.heat_relay.set_low();
        self.led_relay.set_low();
    }
    
    /// Turn all relays on
    pub fn turn_all_on(&mut self) { 
        self.uv1_relay.set_high();
        self.uv2_relay.set_high();
        self.heat_relay.set_high();
        self.led_relay.set_high();
    }
}