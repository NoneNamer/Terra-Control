use modules::config::

use std::thread;
use std::time::Duration;
use chrono::Local;
use rppal::gpio::{Gpio, OutputPin};
use rusqlite::{params, Connection, Result};



//gpio logic with overheat protection
impl LightController {
    fn new(config: LightControlConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let gpio = Gpio::new()?;
        Ok(LightController {
            uv1: gpio.get(config.uv_relay1)?.into_output(),
            uv2: gpio.get(config.uv_relay2)?.into_output(),
            heat: gpio.get(config.heat_relay)?.into_output(),
            overheat_temp: config.overheat_temp,
            overheat_time: Duration::from_secs(config.overheat_time),
            last_overheat: None,
        })
    }

    fn set_uv1(&mut self, state: bool) {
        if state {
            self.uv1.set_high();
        } else {
            self.uv1.set_low();
        }
    }

    fn set_uv2(&mut self, state: bool) {
        if state {
            self.uv2.set_high();
        } else {
            self.uv2.set_low();
        }
    }

    fn control_heat(&mut self, state: bool) {
        let current_temp = get_current_temperature();
        if current_temp >= self.overheat_temp {
            self.set_heat(false);
            self.last_overheat = Some(Instant::now());
        } else if let Some(last_overheat) = self.last_overheat {
            if last_overheat.elapsed() >= self.overheat_time {
                self.set_heat(state);
                self.last_overheat = None;
            } else {
                self.set_heat(false);
            }
        } else {
            self.set_heat(state);
        }
    }
}