//! MQTT blocking client example which subscribes to an internet MQTT server and then sends
//! and receives events in its own topic.

capnp::generated_code!(pub mod message_capnp);

mod game;
mod rgbdriver;

use core::time::Duration;
use std::sync::mpsc;
use std::thread;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::ledc::LedcTimerDriver;
use esp_idf_svc::hal::peripheral::Peripheral;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::hal::*;
use esp_idf_svc::mqtt::client::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::{EspSntp, SyncStatus};
use esp_idf_svc::sys::EspError;
use esp_idf_svc::timer::EspTaskTimerService;
use esp_idf_svc::wifi::*;

use log::*;

use heapless::String;

use crate::game::{GameState, Message};
use crate::rgbdriver::RGBDriver;

// const SSID: &str = "WIFI_SSID";
// const PASSWORD: &str = "WIFI_PASS";

const MQTT_URL: &str = "mqtt://broker.emqx.io:1883";
// const MQTT_URL: &str = "mqtt://public.cez.ar:1883";
const MQTT_CLIENT_ID: &str = "esp-mqtt-demo";
const TOPIC_CLIENTS: &str = "santiagocezar/rgblitz/clients";
const TOPIC_GAME: &str = "santiagocezar/rgblitz/game";

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("hello! this is rgblitz");

    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let peripherals = Peripherals::take().unwrap();

    let timer_driver = RGBDriver::default_timer_driver(peripherals.ledc.timer0).unwrap();

    let mut led1 = RGBDriver::new(
        &timer_driver,
        peripherals.ledc.channel0,
        peripherals.ledc.channel1,
        peripherals.ledc.channel2,
        peripherals.pins.gpio12,
        peripherals.pins.gpio14,
        peripherals.pins.gpio27,
    )
    .unwrap();

    let mut led2 = RGBDriver::new(
        &timer_driver,
        peripherals.ledc.channel3,
        peripherals.ledc.channel4,
        peripherals.ledc.channel5,
        peripherals.pins.gpio25,
        peripherals.pins.gpio33,
        peripherals.pins.gpio32,
    )
    .unwrap();

    let timer_service = EspTaskTimerService::new().unwrap();

    info!("executing cool loading thing...");

    let _wifi = std::thread::scope(|s| {
        let led1 = &mut led1;
        let led2 = &mut led2;

        let (tx, rx) = mpsc::channel::<bool>();

        let callback_timer = {
            let mut state = false;
            timer_service
                .timer(move || {
                    tx.send(state).unwrap();
                    state = !state;
                })
                .unwrap()
        };

        std::thread::Builder::new()
            .stack_size(6000)
            .spawn_scoped(s, move || {
                for state in rx {
                    led1.fade_to(
                        if state { &[200, 50, 150] } else { &[0, 0, 0] },
                        Duration::from_secs(1),
                    )
                    .unwrap();
                    led2.fade_to(
                        if state { &[0, 0, 0] } else { &[200, 150, 50] },
                        Duration::from_secs(1),
                    )
                    .unwrap();
                }
            })
            .unwrap();

        callback_timer.every(Duration::from_secs(1)).unwrap();

        info!("connecting to wifi...");

        let wifi = wifi_create(&sys_loop, &nvs, peripherals.modem).unwrap();

        let ntp = EspSntp::new_default().unwrap();

        info!("syncing clocks...");

        while ntp.get_sync_status() != SyncStatus::Completed {}

        callback_timer.cancel().unwrap();

        wifi
    });

    info!("entering main loop...");

    run(led1, led2).unwrap();
}

fn run(mut led1: RGBDriver, mut led2: RGBDriver) -> Result<(), EspError> {
    let (mut client, mut connection) = mqtt_create(MQTT_URL, MQTT_CLIENT_ID).unwrap();

    std::thread::scope(|s| {
        let mut game = GameState::new();
        game.update_leds(&mut led1, &mut led2)?;

        let (tx, rx) = mpsc::channel::<Message>();

        std::thread::Builder::new()
            .stack_size(6000)
            .spawn_scoped(s, {
                let tx = tx.clone();
                move || {
                    info!("MQTT Listening for messages");

                    while let Ok(event) = connection.next() {
                        let payload = event.payload();
                        info!("[Queue] Event: {}", payload);
                        match payload {
                            EventPayload::Received { data, .. } => {
                                match Message::from_bytes(data) {
                                    Ok(msg) => {
                                        tx.send(msg).unwrap();
                                    }
                                    Err(e) => info!("invalid message: {e}"),
                                }
                            }
                            _ => {}
                        }
                    }

                    info!("Connection closed");
                }
            })
            .unwrap();

        let timer_service = EspTaskTimerService::new()?;
        let callback_timer = {
            timer_service.timer(move || {
                tx.send(Message::Tick).unwrap();
            })?
        };

        // Just to give a chance of our connection to get even the first published message

        std::thread::sleep(Duration::from_millis(500));

        while let Err(e) = client.subscribe(TOPIC_CLIENTS, QoS::AtMostOnce) {
            error!("Failed to subscribe to topic \"{TOPIC_CLIENTS}\": {e}, retrying...");

            // Re-try in 0.5s
            std::thread::sleep(Duration::from_millis(500));

            continue;
        }

        info!("Subscribed to topic \"{TOPIC_CLIENTS}\"");

        callback_timer.every(Duration::from_secs(1))?;

        loop {
            let msg = rx.try_recv().ok();
            let res = if let Some(msg) = msg {
                game.recv(msg)
            } else {
                None
            };

            game.update_leds(&mut led1, &mut led2)?;

            if let Some(res) = res {
                client.enqueue(
                    TOPIC_GAME,
                    QoS::ExactlyOnce,
                    false,
                    &res.as_bytes().unwrap(),
                )?;
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    })
}

fn mqtt_create(
    url: &str,
    client_id: &str,
) -> Result<(EspMqttClient<'static>, EspMqttConnection), EspError> {
    let (mqtt_client, mqtt_conn) = EspMqttClient::new(
        url,
        &MqttClientConfiguration {
            client_id: Some(client_id),
            ..Default::default()
        },
    )?;

    Ok((mqtt_client, mqtt_conn))
}

fn wifi_create(
    sys_loop: &EspSystemEventLoop,
    nvs: &EspDefaultNvsPartition,
    modem: modem::Modem,
) -> Result<EspWifi<'static>, EspError> {
    let mut esp_wifi = EspWifi::new(modem, sys_loop.clone(), Some(nvs.clone()))?;
    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sys_loop.clone())?;

    wifi.start()?;
    info!("Wifi started");

    let networks = wifi.scan()?;

    info!("Found {} networks:", networks.len());

    for network in networks {
        let password = match network.ssid.as_str() {
            "WifiCR" => Some("CR2546938"),
            "UTN_LIBRE" => None,
            _ => {
                continue;
            }
        };

        wifi.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: network.ssid,
            password: password.map_or(String::new(), |s| s.try_into().unwrap()),
            auth_method: if password.is_none() {
                AuthMethod::None
            } else {
                AuthMethod::WPA2Personal
            },
            ..Default::default()
        }))?;

        break;
    }

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(esp_wifi)
}
