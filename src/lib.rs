//! This driver is based on the datasheet which can be found here:
//! https://www.ti.com/lit/ds/symlink/ads1234.pdf?ts=1735781638226

#![no_std]

use core::marker::PhantomData;

use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin, PinState, StatefulOutputPin},
};

#[doc(hidden)]
mod private {
    pub trait Sealed {}
}

pub trait ADSModel: private::Sealed {}

pub struct ADS123X<DOUT, SCLK, PWDN, A0, A1, M>
where
    DOUT: InputPin,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
    M: ADSModel,
{
    dout: DOUT,
    sclk: SCLK,
    pwdn: PWDN,
    a0: A0,
    a1: A1,
    _model: PhantomData<M>,
}

impl<DOUT, SCLK, PWDN, A0, A1, M> ADS123X<DOUT, SCLK, PWDN, A0, A1, M>
where
    DOUT: InputPin,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
    M: ADSModel,
{
    fn new(dout: DOUT, sclk: SCLK, pwdn: PWDN, a0: A0, a1: A1) -> Self {
        Self {
            dout,
            sclk,
            pwdn,
            a0,
            a1,
            _model: PhantomData,
        }
    }

    /// Sets PWDN low, waits for the AVDD voltage to stabilize, then pulses PWDN
    /// once before setting it high
    pub fn reset_blocking(&mut self, delay: &mut impl DelayNs) {
        self.pwdn.set_low().unwrap();

        // Wait for AVDD to stabilize (we can't easily measure this so we just
        // wait for a predefined amount of time that should be fine)
        delay.delay_us(50);

        self.pwdn.set_high().unwrap();
        delay.delay_us(26);

        self.pwdn.set_low().unwrap();
        delay.delay_us(26);

        self.pwdn.set_high().unwrap();
    }

    /// Sets SCLK low, waits for DRDY to go low (blocking), and then pulses the
    /// SCLK 26 times to initiate calibration offset mode
    pub fn calibrate_offset_blocking(&mut self, delay: &mut impl DelayNs) {
        let _ = self.read_internal_blocking(delay);

        // Pulse SCLK a 26th time to start calibration
        self.sclk.set_high().unwrap();
        delay.delay_ns(100);
        self.sclk.set_low().unwrap();

        // Wait for DRDY to go low again which signals that calibration is
        // complete
        while self.dout.is_high().unwrap() {}
    }

    /// Sets SCLK low, waits for DRDY to go low (blocking), and then sets SCLK
    /// high to initiate standby mode (will take 12ms when SPEED is high and
    /// 99ms when speed is low to actually initiate standby)
    pub fn enter_standby_blocking(&mut self) {
        self.sclk.set_low().unwrap();

        while self.dout.is_high().unwrap() {}

        self.sclk.set_high().unwrap();
    }

    /// Sets SCLK low, waits for DRDY to go low (blocking), and then pulses the
    /// SCLK to extract the data from DOUT
    ///
    /// This operation automatically exits standby mode and the first available
    /// data is guaranteed to be valid
    fn read_internal_blocking(&mut self, delay: &mut impl DelayNs) -> i32 {
        self.sclk.set_low().unwrap();

        while self.dout.is_high().unwrap() {}

        let mut data = 0u32;

        for _ in 0..24 {
            self.sclk.set_high().unwrap();
            delay.delay_ns(50);

            data |= self.dout.is_high().unwrap() as u32;
            data <<= 1;

            delay.delay_ns(50);
            self.sclk.set_low().unwrap();
            delay.delay_ns(100);
        }

        // Pulse SCLK a 25th time to force DRDY high
        self.sclk.set_high().unwrap();
        delay.delay_ns(100);
        self.sclk.set_low().unwrap();
        delay.delay_ns(100);

        i24_to_i32(data)
    }
}

#[cfg(feature = "embedded-hal-async")]
impl<DOUT, SCLK, PWDN, A0, A1, M> ADS123X<DOUT, SCLK, PWDN, A0, A1, M>
where
    DOUT: InputPin + embedded_hal_async::digital::Wait,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
    M: ADSModel,
{
    /// Sets PWDN low, waits for the AVDD voltage to stabilize, then pulses PWDN
    /// once before setting it high
    pub async fn reset(&mut self, delay: &mut impl embedded_hal_async::delay::DelayNs) {
        self.pwdn.set_low().unwrap();

        // Wait for AVDD to stabilize (we can't easily measure this so we just
        // wait for a predefined amount of time that should be fine)
        delay.delay_us(50).await;

        self.pwdn.set_high().unwrap();
        delay.delay_us(26).await;

        self.pwdn.set_low().unwrap();
        delay.delay_us(26).await;

        self.pwdn.set_high().unwrap();
    }

    /// Sets SCLK low, waits for DRDY to go low, and then pulses the SCLK 26
    /// times to initiate calibration offset mode
    pub async fn calibrate_offset(&mut self, delay: &mut impl embedded_hal_async::delay::DelayNs) {
        let _ = self.read_internal(delay).await;

        // Pulse SCLK a 26th time to start calibration
        self.sclk.set_high().unwrap();
        delay.delay_ns(100).await;
        self.sclk.set_low().unwrap();

        // Wait for DRDY to go low again which signals that calibration is
        // complete
        self.dout.wait_for_high().await.unwrap();
    }

    /// Sets SCLK low, waits for DRDY to go low, and then sets SCLK high to
    /// initiate standby mode (will take 12ms when SPEED is high and 99ms when
    /// speed is low to actually initiate standby)
    pub async fn enter_standby(&mut self) {
        self.sclk.set_low().unwrap();
        self.dout.wait_for_high().await.unwrap();
        self.sclk.set_high().unwrap();
    }

    /// Sets SCLK low, waits for DRDY to go low, and then pulses the SCLK to
    /// extract the data from DOUT
    ///
    /// This operation automatically exits standby mode and the first available
    /// data is guaranteed to be valid
    async fn read_internal(&mut self, delay: &mut impl embedded_hal_async::delay::DelayNs) -> i32 {
        self.sclk.set_low().unwrap();

        self.dout.wait_for_low().await.unwrap();

        let mut data = 0u32;

        for _ in 0..24 {
            self.sclk.set_high().unwrap();
            delay.delay_ns(50).await;

            data |= self.dout.is_high().unwrap() as u32;
            data <<= 1;

            delay.delay_ns(50).await;
            self.sclk.set_low().unwrap();
            delay.delay_ns(100).await;
        }

        // Pulse SCLK a 25th time to force DRDY high
        self.sclk.set_high().unwrap();
        delay.delay_ns(100).await;
        self.sclk.set_low().unwrap();
        delay.delay_ns(100).await;

        i24_to_i32(data)
    }
}

fn i24_to_i32(value: u32) -> i32 {
    // Mask to get the lower 24 bits
    let masked_value = value & 0xFFFFFF;

    // Check if the 24th bit (sign bit) is set
    if masked_value & 0x800000 != 0 {
        // If so, subtract 2^24 to convert to negative
        masked_value as i32 - (1 << 24)
    } else {
        // Otherwise, just return the value as a positive i32
        masked_value as i32
    }
}

/* ======== ADS1232 ======== */

pub struct ADS1232;

impl private::Sealed for ADS1232 {}
impl ADSModel for ADS1232 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ADS1232Channel {
    AIN1,
    AIN2,
    /// Reads from the internal temperature sensor
    ///
    /// NOTE: Switching from reading any of the other inputs to reading the
    /// temperature sensor requires 4 full conversion cycles before the data is
    /// fully settled. This can be upwards of 250ms if SPEED is enabled, or
    /// 2,000ms if SPEED is disabled.
    Temp,
}

impl ADS1232 {
    pub fn new<DOUT, SCLK, PWDN, A0, A1>(
        dout: DOUT,
        sclk: SCLK,
        pwdn: PWDN,
        a0: A0,
        a1: A1,
    ) -> ADS123X<DOUT, SCLK, PWDN, A0, A1, Self>
    where
        DOUT: InputPin,
        SCLK: OutputPin,
        PWDN: OutputPin,
        A0: StatefulOutputPin,
        A1: StatefulOutputPin,
    {
        ADS123X::new(dout, sclk, pwdn, a0, a1)
    }
}

impl<DOUT, SCLK, PWDN, A0, A1> ADS123X<DOUT, SCLK, PWDN, A0, A1, ADS1232>
where
    DOUT: InputPin,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
{
    /// Sets the new state if it is different from the old state and returns the
    /// old state
    #[must_use]
    fn set_channel(&mut self, channel: ADS1232Channel) -> ADS1232Channel {
        let old_channel = match (
            self.a0.is_set_high().unwrap(),
            self.a1.is_set_high().unwrap(),
        ) {
            (true, false) => ADS1232Channel::AIN1,
            (false, false) => ADS1232Channel::AIN2,
            (_, true) => ADS1232Channel::Temp,
        };

        if channel != old_channel {
            let (a0, temp) = match channel {
                ADS1232Channel::AIN1 => (PinState::Low, PinState::Low),
                ADS1232Channel::AIN2 => (PinState::High, PinState::Low),
                ADS1232Channel::Temp => (PinState::Low, PinState::High),
            };

            self.a0.set_state(a0).unwrap();
            self.a1.set_state(temp).unwrap();
        }

        old_channel
    }

    /// Reads data from the given ADS channel and returns the value decoded as
    /// an i32. If the chip was previously in standby mode, this will exit
    /// standby mode.
    ///
    /// Callers of this function should note that it may block for an
    /// extended period of time (several hundred ms) depending on the configured
    /// SPEED, and the channel being read from.
    ///
    /// If reading from the TEMP channel and the previous read was from a
    /// different channel (or vice versa), this will incur a particularly large
    /// penalty as 4 conversions must be thrown away before the value is
    /// considered settled.
    pub fn read_blocking(&mut self, delay: &mut impl DelayNs, channel: ADS1232Channel) -> i32 {
        let old_channel = self.set_channel(channel);

        // Wait for DRDY setup time if we changed the channel
        if old_channel != channel {
            delay.delay_us(50);
        }

        // Throw away 4 conversions if we changed the value of the TEMP pin
        // (Datasheet section 8.3.7)
        if (old_channel == ADS1232Channel::Temp) != (channel == ADS1232Channel::Temp) {
            for _ in 0..4 {
                self.read_internal_blocking(delay);
            }
        }

        self.read_internal_blocking(delay)
    }
}

#[cfg(feature = "embedded-hal-async")]
impl<DOUT, SCLK, PWDN, A0, A1> ADS123X<DOUT, SCLK, PWDN, A0, A1, ADS1232>
where
    DOUT: InputPin + embedded_hal_async::digital::Wait,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
{
    /// Reads data from the given ADS channel asynchronously and returns the
    /// value decoded as an i32. If the chip was previously in standby mode,
    /// this will exit standby mode.
    ///
    /// Callers of this function should note that it may take an extended period
    /// of time (several hundred ms) for the future to resolve depending on the
    /// configured SPEED, and the channel being read from.
    ///
    /// If reading from the TEMP channel and the previous read was from a
    /// different channel (or vice versa), this will incur a particularly large
    /// penalty as 4 conversions must be thrown away before the value is
    /// considered settled.
    pub async fn read(
        &mut self,
        delay: &mut impl embedded_hal_async::delay::DelayNs,
        channel: ADS1232Channel,
    ) -> i32 {
        let old_channel = self.set_channel(channel);

        // Wait for DRDY setup time if we changed the channel
        if old_channel != channel {
            delay.delay_us(50).await;
        }

        // Throw away 4 conversions if we changed the value of the TEMP pin
        // (Datasheet section 8.3.7)
        if (old_channel == ADS1232Channel::Temp) != (channel == ADS1232Channel::Temp) {
            for _ in 0..4 {
                self.read_internal(delay).await;
            }
        }

        self.read_internal(delay).await
    }
}

/* ======== ADS1234 ======== */

pub struct ADS1234;

impl private::Sealed for ADS1234 {}
impl ADSModel for ADS1234 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ADS1234Channel {
    AIN1,
    AIN2,
    AIN3,
    AIN4,
}

impl ADS1234 {
    pub fn new<DOUT, SCLK, PWDN, A0, A1>(
        dout: DOUT,
        sclk: SCLK,
        pwdn: PWDN,
        a0: A0,
        a1: A1,
    ) -> ADS123X<DOUT, SCLK, PWDN, A0, A1, Self>
    where
        DOUT: InputPin,
        SCLK: OutputPin,
        PWDN: OutputPin,
        A0: StatefulOutputPin,
        A1: StatefulOutputPin,
    {
        ADS123X::new(dout, sclk, pwdn, a0, a1)
    }
}

impl<DOUT, SCLK, PWDN, A0, A1> ADS123X<DOUT, SCLK, PWDN, A0, A1, ADS1234>
where
    DOUT: InputPin,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
{
    fn set_channel(&mut self, channel: ADS1234Channel) -> ADS1234Channel {
        let old_channel = match (
            self.a0.is_set_high().unwrap(),
            self.a1.is_set_high().unwrap(),
        ) {
            (false, false) => ADS1234Channel::AIN1,
            (true, false) => ADS1234Channel::AIN2,
            (false, true) => ADS1234Channel::AIN3,
            (true, true) => ADS1234Channel::AIN4,
        };

        if old_channel != channel {
            let (a0, a1) = match channel {
                ADS1234Channel::AIN1 => (PinState::Low, PinState::Low),
                ADS1234Channel::AIN2 => (PinState::High, PinState::Low),
                ADS1234Channel::AIN3 => (PinState::Low, PinState::High),
                ADS1234Channel::AIN4 => (PinState::High, PinState::High),
            };

            self.a0.set_state(a0).unwrap();
            self.a1.set_state(a1).unwrap();
        }

        old_channel
    }

    /// Reads data from the given ADS channel and returns the value decoded as
    /// an i32. If the chip was previously in standby mode, this will exit
    /// standby mode.
    ///
    /// Callers of this function should note that it may block for an
    /// extended period of time (several hundred ms) depending on the configured
    /// SPEED if waking up from standby mode.
    pub fn read_blocking(&mut self, delay: &mut impl DelayNs, channel: ADS1234Channel) -> i32 {
        let old_channel = self.set_channel(channel);

        // Wait for DRDY setup time if we changed the channel
        if old_channel != channel {
            delay.delay_us(50);
        }

        self.read_internal_blocking(delay)
    }
}

#[cfg(feature = "embedded-hal-async")]
impl<DOUT, SCLK, PWDN, A0, A1> ADS123X<DOUT, SCLK, PWDN, A0, A1, ADS1234>
where
    DOUT: InputPin + embedded_hal_async::digital::Wait,
    SCLK: OutputPin,
    PWDN: OutputPin,
    A0: StatefulOutputPin,
    A1: StatefulOutputPin,
{
    /// Reads data from the given ADS channel asynchronously and returns the
    /// value decoded as an i32. If the chip was previously in standby mode,
    /// this will exit standby mode.
    ///
    /// Callers of this function should note that it may take an extended period
    /// of time (several hundred ms) for the future to resolve depending on the
    /// configured SPEED if waking up from standby mode.
    pub async fn read(
        &mut self,
        delay: &mut impl embedded_hal_async::delay::DelayNs,
        channel: ADS1234Channel,
    ) -> i32 {
        let old_channel = self.set_channel(channel);

        // Wait for DRDY setup time if we changed the channel
        if old_channel != channel {
            delay.delay_us(50).await;
        }

        self.read_internal(delay).await
    }
}
