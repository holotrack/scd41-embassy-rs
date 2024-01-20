use embassy_time::Timer;
use embedded_hal_1::i2c::SevenBitAddress;
use embedded_hal_async::i2c::I2c;
use {defmt_rtt as _, panic_probe as _};

use crate::commands::*;

pub struct SDC41<T>
where
    T: I2c<SevenBitAddress>,
{
    addr: u8,
    i2c: T,

    co2: u16,
    crc_co2: u8,

    temperature: u16,
    crc_temperature: u8,

    humidity: u16,
    crc_humidity: u8,
}

impl<T: I2c<SevenBitAddress>> SDC41<T> {
    pub fn new(addr: u8, i2c: T) -> Self {
        Self {
            addr,
            i2c,
            co2: 0,
            crc_co2: 0,
            temperature: 0,
            crc_temperature: 0,
            humidity: 0,
            crc_humidity: 0,
        }
    }

    pub async fn measurements(&mut self) {
        let mut data = [0u8; 9];

        self.i2c
            .write(self.addr, &STOP_PERIODIC_MEASUREMENT)
            .await
            .unwrap();
        Timer::after_millis(500).await;

        self.i2c
            .write(self.addr, &START_PERIODIC_MESUREMENT)
            .await
            .unwrap();
        Timer::after_secs(5).await;

        self.i2c
            .write_read(self.addr, &READ_MEASUREMENT, &mut data)
            .await
            .unwrap();

        self.co2 = u16::from_be_bytes([data[0], data[1]]);
        self.crc_co2 = data[2];

        self.temperature = u16::from_be_bytes([data[3], data[4]]);
        self.crc_temperature = data[5];

        self.humidity = u16::from_be_bytes([data[6], data[7]]);
        self.crc_humidity = data[8];
    }

    pub fn co2(&self) -> u16 {
        self.co2
    }

    pub fn temperature(&self) -> f32 {
        let temp: f32 = self.temperature as f32 * 175_f32 / 65536_f32 - 48.3_f32;
        temp
    }

    pub fn humidity(&self) -> f32 {
        let humi: f32 = self.humidity as f32 * 100_f32 / 65536_f32;
        humi
    }
}
