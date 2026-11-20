//! The only module that touches hardware (relays, WS2805 strips, 1-Wire, I²C).
//!
//! Everything else talks to the `Hardware` trait. There are two backends:
//! - `SimulatedHardware`: runs anywhere, with a simple thermal model for the sensors
//! - `PiHardware` (Linux only): rppal GPIO (relays, BI lines), rpi_ws281x (WS2805 via PWM+DMA),
//!   sysfs (1-Wire), rppal I²C (SHT3x)

use crate::modules::config::Config;
use crate::modules::models::Rgbww;
use anyhow::Result;
use std::sync::{Arc, Mutex, MutexGuard};

/// Switchable loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relay {
    Uv1,
    Uv2,
    Heat,
    /// Optional relay of the 24 V LED power supply
    LedPsu,
}

impl Relay {
    pub fn name(&self) -> &'static str {
        match self {
            Relay::Uv1 => "UV1",
            Relay::Uv2 => "UV2",
            Relay::Heat => "Heat",
            Relay::LedPsu => "LED-PSU",
        }
    }
}

/// Air temperature and humidity from the SHT3x.
#[derive(Debug, Clone, Copy)]
pub struct Climate {
    pub temperature: f32,
    pub humidity: f32,
}

/// Hardware abstraction. All calls are blocking (sensor reads can take ~1 s);
/// async code must call them via `tokio::task::spawn_blocking`.
pub trait Hardware: Send {
    /// Human-readable backend name ("simulation" / "raspberry-pi")
    fn backend(&self) -> &'static str;
    /// Switches a relay. Switching a relay that is not fitted is a no-op.
    fn set_relay(&mut self, relay: Relay, on: bool) -> Result<()>;
    /// Last state written to a relay.
    fn relay(&self, relay: Relay) -> bool;
    /// Writes one colour to all ICs of both LED strips.
    fn write_leds(&mut self, color: Rgbww) -> Result<()>;
    /// Reads a 1-Wire temperature sensor (°C) by its fixed ID.
    fn read_onewire(&mut self, sensor_id: &str) -> Result<f32>;
    /// Reads the SHT3x.
    fn read_climate(&mut self) -> Result<Climate>;
}

/// Hardware handle shared between the tasks.
#[derive(Clone)]
pub struct SharedHardware(Arc<Mutex<Box<dyn Hardware>>>);

impl SharedHardware {
    pub fn new(hw: Box<dyn Hardware>) -> Self {
        SharedHardware(Arc::new(Mutex::new(hw)))
    }

    /// Locks the hardware. A poisoned lock is recovered: the hardware state itself stays valid.
    pub fn lock(&self) -> MutexGuard<'_, Box<dyn Hardware>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Creates the backend selected in the config. Non-Linux builds always simulate.
pub fn create(config: &Config) -> Result<SharedHardware> {
    #[cfg(target_os = "linux")]
    if !config.hardware.simulate {
        return Ok(SharedHardware::new(Box::new(pi::PiHardware::new(config)?)));
    }
    #[cfg(not(target_os = "linux"))]
    if !config.hardware.simulate {
        eprintln!("hardware.simulate = false is only supported on the Pi, using simulation");
    }
    Ok(SharedHardware::new(Box::new(SimulatedHardware::new(config))))
}

// ===== Simulation =====

/// Simulated hardware. Temperatures follow the heat spot (and a bit the UV lamps) with a
/// first-order lag, so overheat protection and the schedule can be tested on the PC.
pub struct SimulatedHardware {
    uv1: bool,
    uv2: bool,
    heat: bool,
    led_psu: bool,
    has_led_psu: bool,
    leds: Rgbww,
    basking_id: String,
    cool_id: String,
    enclosure_id: String,
    basking: f32,
    cool: f32,
    last_update: std::time::Instant,
    noise_seed: u32,
}

const AMBIENT: f32 = 23.0;
/// Time constant of the simulated basking spot in seconds
const BASKING_TAU: f32 = 900.0;

impl SimulatedHardware {
    pub fn new(config: &Config) -> Self {
        SimulatedHardware {
            uv1: false,
            uv2: false,
            heat: false,
            led_psu: false,
            has_led_psu: config.relays.led_psu.is_some(),
            leds: Rgbww::OFF,
            basking_id: config.sensors.basking_id.clone(),
            cool_id: config.sensors.cool_id.clone(),
            enclosure_id: config.sensors.enclosure_id.clone(),
            basking: AMBIENT,
            cool: AMBIENT,
            last_update: std::time::Instant::now(),
            noise_seed: 0x1234_5678,
        }
    }

    /// Small pseudo-random noise in [-amplitude, amplitude] (xorshift, no extra crate).
    fn noise(&mut self, amplitude: f32) -> f32 {
        let mut x = self.noise_seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_seed = x;
        (x as f32 / u32::MAX as f32 * 2.0 - 1.0) * amplitude
    }

    fn advance(&mut self) {
        let dt = self.last_update.elapsed().as_secs_f32();
        self.last_update = std::time::Instant::now();
        let mut target = AMBIENT;
        if self.heat {
            target += 26.0; // halogen spot alone gets the basking spot to ~49 °C
        }
        if self.uv1 {
            target += 4.0; // metal-halide lamp also heats
        }
        if self.uv2 {
            target += 1.0;
        }
        let k = 1.0 - (-dt / BASKING_TAU).exp();
        self.basking += (target - self.basking) * k;
        let cool_target = AMBIENT + (self.basking - AMBIENT) * 0.15;
        self.cool += (cool_target - self.cool) * k;
    }
}

impl Hardware for SimulatedHardware {
    fn backend(&self) -> &'static str {
        "simulation"
    }

    fn set_relay(&mut self, relay: Relay, on: bool) -> Result<()> {
        self.advance();
        match relay {
            Relay::Uv1 => self.uv1 = on,
            Relay::Uv2 => self.uv2 = on,
            Relay::Heat => self.heat = on,
            Relay::LedPsu if self.has_led_psu => self.led_psu = on,
            Relay::LedPsu => {}
        }
        Ok(())
    }

    fn relay(&self, relay: Relay) -> bool {
        match relay {
            Relay::Uv1 => self.uv1,
            Relay::Uv2 => self.uv2,
            Relay::Heat => self.heat,
            Relay::LedPsu => self.led_psu,
        }
    }

    fn write_leds(&mut self, color: Rgbww) -> Result<()> {
        self.leds = color;
        Ok(())
    }

    fn read_onewire(&mut self, sensor_id: &str) -> Result<f32> {
        self.advance();
        let value = if sensor_id == self.basking_id {
            self.basking + self.noise(0.2)
        } else if sensor_id == self.cool_id {
            self.cool + self.noise(0.1)
        } else if sensor_id == self.enclosure_id {
            AMBIENT + 8.0 + self.noise(0.3)
        } else {
            anyhow::bail!("1-Wire sensor '{sensor_id}' not found");
        };
        Ok((value * 10.0).round() / 10.0)
    }

    fn read_climate(&mut self) -> Result<Climate> {
        self.advance();
        let temperature = AMBIENT + (self.basking - AMBIENT) * 0.3 + self.noise(0.2);
        // Warmer air -> lower relative humidity
        let humidity = 45.0 - (temperature - AMBIENT) * 1.2 + self.noise(1.0);
        Ok(Climate {
            temperature: (temperature * 10.0).round() / 10.0,
            humidity: (humidity.clamp(0.0, 100.0) * 10.0).round() / 10.0,
        })
    }
}

// ===== Protocol helpers (platform independent, unit tested) =====

/// One colour channel of a WS2805 IC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedChannel {
    R,
    G,
    B,
    WarmWhite,
    ColdWhite,
}

/// Parses the byte order of the 5 channels, e.g. "rgbwc" (w = warm white, c = cold white).
pub fn parse_channel_order(s: &str) -> Result<[LedChannel; 5], String> {
    let chars: Vec<char> = s.trim().to_ascii_lowercase().chars().collect();
    if chars.len() != 5 {
        return Err(format!("'{s}' must have exactly 5 letters out of r, g, b, w, c"));
    }
    let mut order = [LedChannel::R; 5];
    for (i, c) in chars.iter().enumerate() {
        order[i] = match c {
            'r' => LedChannel::R,
            'g' => LedChannel::G,
            'b' => LedChannel::B,
            'w' => LedChannel::WarmWhite,
            'c' => LedChannel::ColdWhite,
            other => return Err(format!("'{s}': unknown channel '{other}' (use r, g, b, w, c)")),
        };
        if chars[..i].contains(c) {
            return Err(format!("'{s}': channel '{c}' appears twice"));
        }
    }
    Ok(order)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // used by the Pi backend
fn channel_value(color: Rgbww, channel: LedChannel) -> u8 {
    match channel {
        LedChannel::R => color.r,
        LedChannel::G => color.g,
        LedChannel::B => color.b,
        LedChannel::WarmWhite => color.ww,
        LedChannel::ColdWhite => color.cw,
    }
}

/// Packs `ic_count` WS2805 ICs (5 bytes each) into 3-byte "RGB pixels" for rpi_ws281x.
///
/// rpi_ws281x only knows 3/4-byte pixels. With strip type WS2811_STRIP_RGB it sends the bytes
/// of each pixel as R, G, B, so the 40-bit WS2805 frames can be laid out back to back. The last
/// pixel is padded with zeros; those bits run past the last IC and are ignored.
/// Returns 0x00RRGGBB words (the layout of `ws2811_led_t`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // used by the Pi backend
pub fn pack_ws2805(color: Rgbww, ic_count: usize, order: &[LedChannel; 5]) -> Vec<u32> {
    let frame: Vec<u8> = order.iter().map(|ch| channel_value(color, *ch)).collect();
    let bytes: Vec<u8> = frame.iter().copied().cycle().take(ic_count * 5).collect();
    bytes
        .chunks(3)
        .map(|c| {
            let get = |i: usize| c.get(i).copied().unwrap_or(0) as u32;
            (get(0) << 16) | (get(1) << 8) | get(2)
        })
        .collect()
}

/// Number of rpi_ws281x pixels needed for `ic_count` WS2805 ICs.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // used by the Pi backend
pub fn ws2805_pixel_count(ic_count: usize) -> usize {
    (ic_count * 5).div_ceil(3)
}

/// CRC-8 of the SHT3x (polynomial 0x31, init 0xFF).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // used by the Pi backend
pub fn sht3x_crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0xFF;
    for byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x31 } else { crc << 1 };
        }
    }
    crc
}

/// Decodes the 6-byte SHT3x measurement (T msb, T lsb, CRC, RH msb, RH lsb, CRC).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // used by the Pi backend
pub fn sht3x_decode(buf: &[u8; 6]) -> Result<Climate> {
    if sht3x_crc8(&buf[0..2]) != buf[2] || sht3x_crc8(&buf[3..5]) != buf[5] {
        anyhow::bail!("SHT3x CRC mismatch");
    }
    let raw_t = u16::from_be_bytes([buf[0], buf[1]]) as f32;
    let raw_rh = u16::from_be_bytes([buf[3], buf[4]]) as f32;
    Ok(Climate {
        temperature: -45.0 + 175.0 * raw_t / 65535.0,
        humidity: (100.0 * raw_rh / 65535.0).clamp(0.0, 100.0),
    })
}

/// Parses the classic `w1_slave` file of a DS18B20 / MAX31820 (°C).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))] // used by the Pi backend
pub fn parse_w1_slave(text: &str) -> Result<f32> {
    let mut lines = text.lines();
    let crc_line = lines.next().unwrap_or_default();
    if !crc_line.trim_end().ends_with("YES") {
        anyhow::bail!("1-Wire CRC check failed");
    }
    let value = lines
        .next()
        .and_then(|l| l.split("t=").nth(1))
        .ok_or_else(|| anyhow::anyhow!("no temperature in w1_slave"))?;
    let milli: i32 = value.trim().parse()?;
    Ok(milli as f32 / 1000.0)
}

// ===== Raspberry Pi =====

#[cfg(target_os = "linux")]
mod pi {
    use super::*;
    use anyhow::{bail, Context};
    use rppal::gpio::{Gpio, OutputPin};
    use rppal::i2c::I2c;
    use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType};
    use std::time::Duration;

    /// DMA channel used by rpi_ws281x (10 is the recommended free channel)
    const DMA_CHANNEL: i32 = 10;
    /// SHT3x single shot, high repeatability, no clock stretching
    const SHT3X_MEASURE: [u8; 2] = [0x24, 0x00];

    /// rpi_ws281x controller. It holds raw pointers and is therefore not `Send`; all access
    /// goes through the `SharedHardware` mutex, so moving it between threads is fine.
    struct LedOutput {
        controller: Controller,
        order: [LedChannel; 5],
        ic_counts: [usize; 2],
    }

    // SAFETY: the controller is only ever used by one thread at a time (behind the hardware
    // mutex) and rpi_ws281x keeps no thread-local state.
    unsafe impl Send for LedOutput {}

    impl LedOutput {
        fn new(config: &Config) -> Result<Self> {
            let led = &config.led;
            let order = parse_channel_order(&led.channel_order).map_err(anyhow::Error::msg)?;
            let channel = |pin: u8, ics: usize| {
                let (pin, count) = if ics == 0 { (0, 0) } else { (pin as i32, ws2805_pixel_count(ics) as i32) };
                ChannelBuilder::new()
                    .pin(pin)
                    .count(count)
                    .strip_type(StripType::Ws2811Rgb)
                    .brightness(255) // 255 = no scaling in rpi_ws281x
                    .build()
            };
            let controller = ControllerBuilder::new()
                .freq(led.frequency_hz)
                .dma(DMA_CHANNEL)
                .channel(0, channel(led.data_pin_strip1, led.ic_count_strip1))
                .channel(1, channel(led.data_pin_strip2, led.ic_count_strip2))
                .build()
                .map_err(|e| anyhow::anyhow!("rpi_ws281x init failed: {e:?} (running as root? audio off?)"))?;
            Ok(LedOutput { controller, order, ic_counts: [led.ic_count_strip1, led.ic_count_strip2] })
        }

        fn write(&mut self, color: Rgbww) -> Result<()> {
            for (channel, ics) in self.ic_counts.into_iter().enumerate() {
                if ics == 0 {
                    continue;
                }
                let words = pack_ws2805(color, ics, &self.order);
                for (led, word) in self.controller.leds_mut(channel).iter_mut().zip(words) {
                    // ws2811_led_t is a native-endian u32 (0x00RRGGBB)
                    *led = word.to_ne_bytes();
                }
            }
            self.controller.render().map_err(|e| anyhow::anyhow!("LED render failed: {e:?}"))?;
            self.controller.wait().map_err(|e| anyhow::anyhow!("LED render wait failed: {e:?}"))?;
            Ok(())
        }
    }

    pub struct PiHardware {
        uv1: OutputPin,
        uv2: OutputPin,
        heat: OutputPin,
        led_psu: Option<OutputPin>,
        /// WS2805 BI lines, kept low for the lifetime of the program
        _backup_pins: [OutputPin; 2],
        leds: Option<LedOutput>,
        i2c: I2c,
    }

    impl PiHardware {
        pub fn new(config: &Config) -> Result<Self> {
            let gpio = Gpio::new().context("failed to open GPIO")?;
            // Active-high relays: claim the pins as outputs that start low (= off)
            let output = |pin: u8| -> Result<OutputPin> {
                Ok(gpio.get(pin).with_context(|| format!("GPIO {pin} unavailable"))?.into_output_low())
            };
            let uv1 = output(config.relays.uv1)?;
            let uv2 = output(config.relays.uv2)?;
            let heat = output(config.relays.heat)?;
            let led_psu = config.relays.led_psu.map(output).transpose()?;
            let backup_pins = [output(config.led.backup_pin_strip1)?, output(config.led.backup_pin_strip2)?];

            let leds = if config.led.ic_count_strip1 == 0 {
                None
            } else {
                Some(LedOutput::new(config)?)
            };

            let mut i2c = I2c::new().context("failed to open I2C bus 1 (dtparam=i2c_arm=on?)")?;
            i2c.set_slave_address(config.sensors.sht3x_address)
                .with_context(|| format!("invalid SHT3x address {:#x}", config.sensors.sht3x_address))?;

            Ok(PiHardware { uv1, uv2, heat, led_psu, _backup_pins: backup_pins, leds, i2c })
        }

        fn pin(&mut self, relay: Relay) -> Option<&mut OutputPin> {
            match relay {
                Relay::Uv1 => Some(&mut self.uv1),
                Relay::Uv2 => Some(&mut self.uv2),
                Relay::Heat => Some(&mut self.heat),
                Relay::LedPsu => self.led_psu.as_mut(),
            }
        }
    }

    impl Hardware for PiHardware {
        fn backend(&self) -> &'static str {
            "raspberry-pi"
        }

        fn set_relay(&mut self, relay: Relay, on: bool) -> Result<()> {
            if let Some(pin) = self.pin(relay) {
                if on {
                    pin.set_high();
                } else {
                    pin.set_low();
                }
            }
            Ok(())
        }

        fn relay(&self, relay: Relay) -> bool {
            match relay {
                Relay::Uv1 => self.uv1.is_set_high(),
                Relay::Uv2 => self.uv2.is_set_high(),
                Relay::Heat => self.heat.is_set_high(),
                Relay::LedPsu => self.led_psu.as_ref().is_some_and(|p| p.is_set_high()),
            }
        }

        fn write_leds(&mut self, color: Rgbww) -> Result<()> {
            match self.leds.as_mut() {
                Some(leds) => leds.write(color),
                None => Ok(()), // no strip configured
            }
        }

        fn read_onewire(&mut self, sensor_id: &str) -> Result<f32> {
            // w1-gpio overlay on GPIO26. Newer kernels offer `temperature` (millidegrees);
            // fall back to the classic `w1_slave` file with CRC check.
            let dir = format!("/sys/bus/w1/devices/{sensor_id}");
            let celsius = match std::fs::read_to_string(format!("{dir}/temperature")) {
                Ok(raw) => {
                    let milli: i32 = raw.trim().parse().with_context(|| format!("invalid value in {dir}/temperature"))?;
                    milli as f32 / 1000.0
                }
                Err(_) => {
                    let text = std::fs::read_to_string(format!("{dir}/w1_slave"))
                        .with_context(|| format!("1-Wire sensor {sensor_id} not found"))?;
                    parse_w1_slave(&text)?
                }
            };
            // 85 °C is the power-on value and means "no conversion happened"
            if celsius == 85.0 {
                bail!("sensor {sensor_id} returned its power-on value (85 °C)");
            }
            if !(-55.0..=125.0).contains(&celsius) {
                bail!("sensor {sensor_id} returned an impossible value ({celsius} °C)");
            }
            Ok(celsius)
        }

        fn read_climate(&mut self) -> Result<Climate> {
            self.i2c.write(&SHT3X_MEASURE).context("SHT3x: measure command failed")?;
            // High repeatability needs up to 15 ms
            std::thread::sleep(Duration::from_millis(20));
            let mut buf = [0u8; 6];
            self.i2c.read(&mut buf).context("SHT3x: read failed")?;
            sht3x_decode(&buf)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_order_parsing() {
        assert_eq!(
            parse_channel_order("RGBwc").unwrap(),
            [LedChannel::R, LedChannel::G, LedChannel::B, LedChannel::WarmWhite, LedChannel::ColdWhite]
        );
        assert!(parse_channel_order("rgbw").is_err());
        assert!(parse_channel_order("rgbwx").is_err());
        assert!(parse_channel_order("rrbwc").is_err());
    }

    #[test]
    fn ws2805_packing_lays_frames_back_to_back() {
        let order = parse_channel_order("grbwc").unwrap();
        let color = Rgbww { r: 1, g: 2, b: 3, ww: 4, cw: 5 };
        // 2 ICs = 10 bytes: 2 1 3 4 5 2 1 3 4 5 -> 4 pixels, last one padded
        let words = pack_ws2805(color, 2, &order);
        assert_eq!(words, vec![0x020103, 0x040502, 0x010304, 0x050000]);
        assert_eq!(ws2805_pixel_count(2), 4);
        assert_eq!(ws2805_pixel_count(3), 5);
        assert_eq!(pack_ws2805(color, 3, &order).len(), 5);
        assert!(pack_ws2805(color, 0, &order).is_empty());
    }

    #[test]
    fn sht3x_crc_matches_datasheet_example() {
        // Datasheet: CRC of 0xBEEF is 0x92
        assert_eq!(sht3x_crc8(&[0xBE, 0xEF]), 0x92);
    }

    #[test]
    fn sht3x_decoding() {
        // raw T = 0x6666 -> 25.0 °C, raw RH = 0x8000 -> 50.0 %
        let t = [0x66, 0x66];
        let rh = [0x80, 0x00];
        let buf = [t[0], t[1], sht3x_crc8(&t), rh[0], rh[1], sht3x_crc8(&rh)];
        let c = sht3x_decode(&buf).unwrap();
        assert!((c.temperature - 25.0).abs() < 0.01, "{}", c.temperature);
        assert!((c.humidity - 50.0).abs() < 0.01, "{}", c.humidity);
        let mut bad = buf;
        bad[2] ^= 1;
        assert!(sht3x_decode(&bad).is_err());
    }

    #[test]
    fn w1_slave_parsing() {
        let ok = "72 01 4b 46 7f ff 0e 10 57 : crc=57 YES\n72 01 4b 46 7f ff 0e 10 57 t=23125\n";
        assert_eq!(parse_w1_slave(ok).unwrap(), 23.125);
        let bad = "72 01 4b 46 7f ff 0e 10 57 : crc=57 NO\n72 01 4b 46 7f ff 0e 10 57 t=23125\n";
        assert!(parse_w1_slave(bad).is_err());
        let negative = "aa : crc=aa YES\naa t=-1250\n";
        assert_eq!(parse_w1_slave(negative).unwrap(), -1.25);
    }
}
