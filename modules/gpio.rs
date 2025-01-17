use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use std::error::Error;
use std::thread;
use std::time::Duration;
use crate::modules::config::GpioConfig

// WS2805 constants (3.2MHz SPI -> 312.5ns per bit)
const T0H: u8 = 0b10000000; // ~312.5ns high
const T1H: u8 = 0b11000000; // ~625ns high
const RESET_TIME_US: u64 = 300; // >280µs reset time
const CHANNELS_PER_IC: usize = 5;  // Each WS2805 controls 5 LED channels
const BITS_PER_CHANNEL: usize = 8; // 8 bits per channel

// Settings to move to config
const IC_COUNT: usize = 16;    // Number of WS2805 ICs in the chain

#[derive(Debug, Clone, Copy)]
struct RGBWW {
    r: u8,
    g: u8,
    b: u8,
    ww: u8,
    cw: u8,
}

impl RGBWW {
    fn off() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            ww: 0,
            cw: 0,
        }
    }

    fn from_str(s: &str) -> Result<Self, Box<dyn Error>> {
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

fn convert_byte(byte: u8, buffer: &mut [u8]) {
    let mut byte = byte;
    for i in 0..8 {
        buffer[i] = if (byte & 0x80) != 0 { T1H } else { T0H };
        byte <<= 1;
    }
}

struct LEDStrip {
    spi: Spi,
    buffer: Vec<u8>,
}

impl LEDStrip {
    fn new() -> Result<Self, Box<dyn Error>> {
        let spi = Spi::new(
            Bus::Spi0,
            SlaveSelect::Ss0,
            3_200_000, // 3.2MHz for correct timing
            Mode::Mode0,
        )?;

        let buffer = vec![0; IC_COUNT * CHANNELS_PER_IC * BITS_PER_CHANNEL];
        Ok(Self { spi, buffer })
    }

    fn set_all(&mut self, color: RGBWW) {
        for i in 0..IC_COUNT {
            self.set_ic(i, color);
        }
    }

    fn set_ic(&mut self, index: usize, color: RGBWW) {
        if index >= IC_COUNT {
            return;
        }

        let start = index * CHANNELS_PER_IC * BITS_PER_CHANNEL;
        convert_byte(color.g, &mut self.buffer[start..start + 8]);
        convert_byte(color.r, &mut self.buffer[start + 8..start + 16]);
        convert_byte(color.b, &mut self.buffer[start + 16..start + 24]);
        convert_byte(color.ww, &mut self.buffer[start + 24..start + 32]);
        convert_byte(color.cw, &mut self.buffer[start + 32..start + 40]);
    }

    fn show(&mut self) -> Result<(), Box<dyn Error>> {
        self.spi.write(&self.buffer)?;
        thread::sleep(Duration::from_micros(RESET_TIME_US));
        Ok(())
    }
}