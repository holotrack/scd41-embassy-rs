#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use core::str::from_utf8;

use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::adc::Async;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::I2C1;
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, i2c};
use embassy_time::{Duration, Timer};
use embedded_hal_1::i2c::SevenBitAddress;
use embedded_hal_1::i2c::{Error, ErrorType};
// use embedded_hal_async::i2c::I2c;
use embassy_rp::i2c::I2c;
use embedded_io_async::Write;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use core::ops::Deref;
use heapless::Vec;
use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};

extern crate scd41_embassy_rs;

use defmt::*;
use scd41_embassy_rs::scd41::SCD41;

const ADDR: u16 = 0x62;

const WIFI_NETWORK: &str = "SilesianCloud-guest";
const WIFI_PASSWORD: &str = "T@jlandia123qwe";

bind_interrupts!(struct Irqs {
    I2C1_IRQ => embassy_rp::i2c::InterruptHandler<I2C1>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn measurments_task(sensor: &'static mut SCD41) -> ! {
    sensor.measurements().await
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Measurments {
    cotwo: u16,
    temp: f32,
    humdt: f32,
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    embassy_rp::pac::SIO.spinlock(31).write_value(1);

    // I2C
    let p = embassy_rp::init(Default::default());

    let sda = p.PIN_18;
    let scl = p.PIN_19;

    info!("set up i2c ");

    static SCD41: StaticCell<SCD41> = StaticCell::new();

    let i2c: I2c<'_, I2C1, i2c::Async> =
        i2c::I2c::new_async(p.I2C1, scl, sda, Irqs, i2c::Config::default());

    let scd41: &'static mut SCD41 = SCD41.init(SCD41::new(ADDR, i2c));

    // WiFi

    let fw = include_bytes!("../../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../../cyw43-firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 69, 2), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(192, 168, 69, 1)),
    //});

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    // Init network stack
    static STACK: StaticCell<Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::<2>::new()),
        seed,
    ));

    unwrap!(spawner.spawn(net_task(stack)));

    loop {
        //control.join_open(WIFI_NETWORK).await;
        match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut buf: [u8; 4096] = [0; 4096];

    unwrap!(spawner.spawn(measurments_task(scd41))); //tutaj trzeba cos wykombinowac, potrzebuje static (cale zycie programu lifetime i mutable)
    println!("after");

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        control.gpio_set(0, false).await;
        info!("Listening on TCP:1234...");
        if let Err(e) = socket.accept(1234).await {
            warn!("accept error: {:?}", e);
            continue;
        }

        info!("Received connection from {:?}", socket.remote_endpoint());
        control.gpio_set(0, true).await;

        loop {
            // let n = match socket.read(&mut buf).await {
            //     Ok(0) => {
            //         warn!("read EOF");
            //         break;
            //     }
            //     Ok(n) => n,
            //     Err(e) => {
            //         warn!("read error: {:?}", e);
            //         break;
            //     }
            // };

            // info!("rxd {}", from_utf8(&buf[..n]).unwrap());

            // sdc41.measurements().await;
            let co2 = scd41.co2();
            // let humidity = scd41.humidity();
            // let temerature = scd41.temperature();
            info!("CO2: {}", co2);

            // let data = to_slice(
            //     &Measurments {
            //         cotwo: co2,
            //         temp: temerature,
            //         humdt: humidity,
            //     },
            //     &mut buf,
            // )
            // .unwrap();

            let data = b"Hello world!";

            match socket.write_all(data).await {
                Ok(()) => {}
                Err(e) => {
                    warn!("write error: {:?}", e);
                    break;
                }
            };
        }
    }
}
