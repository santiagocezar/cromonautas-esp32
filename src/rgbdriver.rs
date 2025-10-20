use std::time::Duration;

use esp_idf_svc::{
    hal::{peripheral::Peripheral, *},
    sys::EspError,
};

pub struct RGBDriver<'d> {
    r: ledc::LedcDriver<'d>,
    b: ledc::LedcDriver<'d>,
    g: ledc::LedcDriver<'d>,
}

impl<'d> RGBDriver<'d> {
    pub fn new<T: ledc::LedcTimer + 'd>(
        timer_driver: &ledc::LedcTimerDriver<'d, T>,
        r_chan: impl Peripheral<P = impl ledc::LedcChannel<SpeedMode = T::SpeedMode>> + 'd,
        g_chan: impl Peripheral<P = impl ledc::LedcChannel<SpeedMode = T::SpeedMode>> + 'd,
        b_chan: impl Peripheral<P = impl ledc::LedcChannel<SpeedMode = T::SpeedMode>> + 'd,
        r_pin: impl Peripheral<P = impl gpio::OutputPin> + 'd,
        g_pin: impl Peripheral<P = impl gpio::OutputPin> + 'd,
        b_pin: impl Peripheral<P = impl gpio::OutputPin> + 'd,
    ) -> Result<Self, EspError> {
        Ok(Self {
            r: ledc::LedcDriver::new(r_chan, timer_driver, r_pin)?,
            g: ledc::LedcDriver::new(g_chan, timer_driver, g_pin)?,
            b: ledc::LedcDriver::new(b_chan, timer_driver, b_pin)?,
        })
    }

    pub fn default_timer_driver<T: ledc::LedcTimer + 'd>(
        timer: impl Peripheral<P = T> + 'd,
    ) -> Result<ledc::LedcTimerDriver<'d, T>, EspError> {
        ledc::LedcTimerDriver::new(
            timer,
            &ledc::config::TimerConfig {
                frequency: units::Hertz(5000),
                resolution: ledc::Resolution::Bits8,
            },
        )
    }

    pub fn set(&mut self, color: &[u8; 3]) -> Result<(), EspError> {
        let [r, g, b] = colorutils::linear_to_srgb(color);
        self.r.set_duty(1 + r as u32)?;
        self.g.set_duty(1 + g as u32)?;
        self.b.set_duty(1 + b as u32)?;
        Ok(())
    }

    pub fn fade_to(&mut self, color: &[u8; 3], time: Duration) -> Result<(), EspError> {
        let [r, g, b] = colorutils::linear_to_srgb(color);
        self.r
            .fade_with_time(r.into(), time.as_millis().try_into().unwrap(), false)?;
        self.g
            .fade_with_time(g.into(), time.as_millis().try_into().unwrap(), false)?;
        self.b
            .fade_with_time(b.into(), time.as_millis().try_into().unwrap(), false)?;
        Ok(())
    }

    pub fn set_raw(&mut self, &[r, g, b]: &[u8; 3]) -> Result<(), EspError> {
        self.r.set_duty(255 - r as u32)?;
        self.g.set_duty(255 - g as u32)?;
        self.b.set_duty(255 - b as u32)?;
        Ok(())
    }
}
