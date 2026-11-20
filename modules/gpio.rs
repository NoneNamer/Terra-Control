//! The only module that touches hardware (relays, WS2805 strips, 1-Wire, I²C).
//!
//! Everything else talks to the `Hardware` trait. There are two backends:
//! - `SimulatedHardware`: runs anywhere, with a simple thermal model for the sensors
//! - `PiHardware` (Linux only): rppal GPIO for the relays, sysfs for 1-Wire

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

// ===== Raspberry Pi =====

#[cfg(target_os = "linux")]
mod pi {
    use super::*;
    use anyhow::{bail, Context};
    use rppal::gpio::{Gpio, OutputPin};

    pub struct PiHardware {
        uv1: OutputPin,
        uv2: OutputPin,
        heat: OutputPin,
        led_psu: Option<OutputPin>,
        leds: Rgbww,
    }

    impl PiHardware {
        pub fn new(config: &Config) -> Result<Self> {
            let gpio = Gpio::new().context("failed to open GPIO")?;
            // Active-high relays: claim the pins as outputs that start low (= off)
            let output = |pin: u8| -> Result<OutputPin> {
                Ok(gpio.get(pin).with_context(|| format!("GPIO {pin} unavailable"))?.into_output_low())
            };
            let hw = PiHardware {
                uv1: output(config.relays.uv1)?,
                uv2: output(config.relays.uv2)?,
                heat: output(config.relays.heat)?,
                led_psu: config.relays.led_psu.map(output).transpose()?,
                leds: Rgbww::OFF,
            };
            eprintln!("WS2805 driver and SHT3x are not implemented yet (phase 2)");
            Ok(hw)
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
            // TODO(phase 2): WS2805 output on GPIO18/13 (PWM)
            self.leds = color;
            Ok(())
        }

        fn read_onewire(&mut self, sensor_id: &str) -> Result<f32> {
            // w1-gpio overlay on GPIO26; the kernel exposes millidegrees Celsius
            let path = format!("/sys/bus/w1/devices/{sensor_id}/temperature");
            let raw = std::fs::read_to_string(&path).with_context(|| format!("failed to read {path}"))?;
            let milli: i32 = raw.trim().parse().with_context(|| format!("invalid value in {path}"))?;
            // 85 °C is the DS18B20 power-on value and means "no conversion happened"
            if milli == 85_000 {
                bail!("sensor {sensor_id} returned its power-on value (85 °C)");
            }
            Ok(milli as f32 / 1000.0)
        }

        fn read_climate(&mut self) -> Result<Climate> {
            // TODO(phase 2): SHT3x over I²C
            bail!("SHT3x driver not implemented yet")
        }
    }
}
