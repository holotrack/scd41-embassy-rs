use embassy_rp::i2c::Async;
use embassy_rp::i2c::I2c;
use embassy_rp::peripherals::I2C1;
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

use crate::commands::*;

pub struct SCD41 {
    addr: u16,
    i2c: I2c<'static, I2C1, Async>,

    co2: u16,
    crc_co2: u8,

    temperature: u16,
    crc_temperature: u8,

    humidity: u16,
    crc_humidity: u8,
}

impl SCD41 {
    pub fn new(addr: u16, i2c: I2c<'static, I2C1, Async>) -> Self {
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
            .write_async(self.addr, STOP_PERIODIC_MEASUREMENT)
            .await
            .unwrap();
        Timer::after_millis(500).await;

        self.i2c
            .write_async(self.addr, START_PERIODIC_MESUREMENT)
            .await
            .unwrap();
        Timer::after_secs(5).await;

        self.i2c
            .write_read_async(self.addr, READ_MEASUREMENT, &mut data)
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
        let temp: f32 = self.temperature as f32 * 175_f32 / 65536_f32 - 45_f32;
        temp
    }

    pub fn humidity(&self) -> f32 {
        let humi: f32 = self.humidity as f32 * 100_f32 / 65536_f32;
        humi
    }
}

mod test {
    // Import all functions to test module

    // #[test]
    // fn task1_works() {
    //     //    assert_eq!(task1(), "Accomplished task 1!".to_string() );
    // }

    // #[test]
    // fn task2_works() {
    //     //    assert_eq!(task3(), "Accomplished task 2!".to_string() );
    // }
}
