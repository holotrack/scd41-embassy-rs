#![no_std]
#![no_main]

extern crate scd41_embassy_rs;

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::i2c::{self, Config, InterruptHandler};
use embassy_rp::peripherals::I2C1;
use scd41_embassy_rs::sdc41::SDC41;

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<I2C1>;
});

pub const ADDR: u8 = 0x62;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    embassy_rp::pac::SIO.spinlock(31).write_value(1);

    let p = embassy_rp::init(Default::default());

    let sda = p.PIN_18;
    let scl = p.PIN_19;

    info!("set up i2c ");

    let i2c = i2c::I2c::new_async(p.I2C1, scl, sda, Irqs, Config::default());

    let mut sdc41 = SDC41::new(ADDR, i2c);

    loop {
        sdc41.measurements().await;
        let co2 = sdc41.co2();
        let humidity = sdc41.humidity();
        let temerature = sdc41.temperature();
        info!("CO2: {}, TEMP: {}, HUMIDITY: {}", co2, temerature, humidity);
    }
}
