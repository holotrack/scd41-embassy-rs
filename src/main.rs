#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::I2c;
use embassy_rp::peripherals::I2C1;
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0};
use embassy_rp::pio::{Pio};
use embassy_rp::{bind_interrupts, i2c};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use postcard::to_slice;
use serde::{Deserialize, Serialize};

use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;

extern crate scd41_embassy_rs;

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
    runner: cyw43::Runner<
        'static,
        Output<'static, PIN_23>,
        PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>,
    >,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Measurements {
    cotwo: u16,
    temp: f32,
    humdt: f32,
}

static SHARED: Mutex<ThreadModeRawMutex, Measurements> = Mutex::new(Measurements {
    cotwo: 0,
    temp: 0_f32,
    humdt: 0_f32,
});

#[embassy_executor::task]
async fn measurments_task(sensor: &'static mut SCD41) -> ! {
    loop {
        debug!("MEASURING IN TASK");
        sensor.measurements().await;
        debug!("LOCKING IN TASK");

        let mut shared = SHARED.lock().await;
        *shared = Measurements {
            cotwo: sensor.co2(),
            humdt: sensor.humidity(),
            temp: sensor.temperature(),
        };
        debug!("UNLOCKING IN TASK");
    }
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

    let scd41 = SCD41.init(SCD41::new(ADDR, i2c));

    // WiFi

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

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

    let mut dhcp = embassy_net::DhcpConfig::default();
    dhcp.hostname = Some(heapless::String::try_from("scd41-sensor").unwrap());
    let config = Config::dhcpv4(dhcp);
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

    unwrap!(spawner.spawn(measurments_task(scd41)));
    println!("after");
    // debug!("BEFORE GPIO SET IN MAIN");
    // control.gpio_set(0, false).await;
    // debug!("AFTER GPIO SET IN MAIN");

    loop {
        debug!("SOCKET IN MAIN");
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        debug!("AFTER SOCKET IN MAIN");

        socket.set_timeout(Some(Duration::from_secs(10)));
        debug!("AFTER TIMEOUT IN MAIN");

        debug!("BEFORE SOCKET.ACCEPT SET IN MAIN");

        info!("Listening on TCP:1234...");
        if let Err(e) = socket.accept(1234).await {
            warn!("accept error: {:?}", e);
            continue;
        }

        debug!("AFTER SOCKET.ACCEPT SET IN MAIN");

        info!("Received connection from {:?}", socket.remote_endpoint());
        // control.gpio_set(0, true).await;
        debug!("LOCKING IN MAIN");
        {
            let shared = SHARED.lock().await;

            debug!("AFTER LOCKING IN MAIN");

            let co2 = shared.cotwo;
            let humidity = shared.humdt;
            let temerature = shared.temp;
            info!("CO2: {}, TEMP: {}, HUMIDITY: {}", co2, temerature, humidity);

            let data = to_slice(
                &Measurements {
                    cotwo: co2,
                    temp: temerature,
                    humdt: humidity,
                },
                &mut buf,
            )
            .unwrap();
            debug!("AFTER DATA SERIALIZATION IN MAIN");
            debug!("WRITE ALL LOOP IN MAIN");
            loop {
                match socket.write_all(data).await {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("write error: {:?}", e);
                        break;
                    }
                };
            }
            debug!("AFTER WRITE ALL LOOP IN MAIN");

            // debug!("BEFORE GPIO SET IN LOOP");
            // control.gpio_set(0, false).await;
            // debug!("AFTER GPIO SET IN LOOP");
        }
    }
}
