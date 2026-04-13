use std::{collections::HashMap, time::SystemTime};

use chrono::{DateTime, Timelike, Utc};
use core_json::{ConstStack, Deserializer};
use esp_idf_svc::sys::{esp_random, EspError};
// use heapless::Vec;
use log::*;

use crate::rgbdriver::RGBDriver;

use crate::message_capnp::{color, message, round_config};
use capnp::serialize_packed;

struct RoundConfig {
    difficulty: u8,
    remaining: u8,
    secret: [u8; 3],
    now: DateTime<Utc>,
    restart: bool,
}

pub enum Message {
    Hello,
    RoundConfig(RoundConfig),
    GuessRequest {
        client: ClientID,
        rgb: [u8; 3],
    },
    GuessResponse {
        client: ClientID,
        rgb: [u8; 3],
        closeness: u8,
        closest: bool,
    },
    Tick,
}

impl Message {
    pub fn from_bytes(b: &[u8]) -> ::capnp::Result<Self> {
        let reader = serialize_packed::read_message(b, ::capnp::message::ReaderOptions::new())?;

        let root = reader.get_root::<message::Reader>()?;

        let res = match root.which()? {
            message::Hello(()) => Self::Hello,
            message::RoundConfig(config) => {
                let config = config?;
                let secret = config.reborrow().get_secret()?;

                Self::RoundConfig(RoundConfig {
                    difficulty: config.get_difficulty(),
                    remaining: config.get_remaining(),
                    secret: [secret.get_r(), secret.get_g(), secret.get_b()],
                    now: DateTime::<Utc>::from_timestamp_millis(config.get_now()).unwrap(),
                    restart: config.get_restart(),
                })
            }
            message::GuessReq(req) => {
                let rgb = req.reborrow().get_rgb()?;

                Self::GuessRequest {
                    client: req.get_client()?.to_string()?,
                    rgb: [rgb.get_r(), rgb.get_g(), rgb.get_b()],
                }
            }
            message::GuessRes(res) => {
                let rgb = res.reborrow().get_rgb()?;

                Self::GuessResponse {
                    client: res.get_client()?.to_string()?,
                    rgb: [rgb.get_r(), rgb.get_g(), rgb.get_b()],
                    closeness: res.get_closeness(),
                    closest: res.get_closest(),
                }
            }
        };

        Ok(res)
    }
    pub fn as_bytes(self) -> ::capnp::Result<Vec<u8>> {
        let mut serialized = ::capnp::message::Builder::new_default();

        let mut root = serialized.init_root::<message::Builder>();

        match self {
            Message::Hello => root.set_hello(()),
            Message::RoundConfig(orig_config) => {
                let mut config = root.init_round_config();

                let mut color = config.reborrow().init_secret();

                color.set_r(orig_config.secret[0]);
                color.set_g(orig_config.secret[1]);
                color.set_b(orig_config.secret[2]);

                config.set_difficulty(orig_config.difficulty);
                config.set_remaining(orig_config.remaining);
                config.set_now(orig_config.now.timestamp_millis());
                config.set_restart(orig_config.restart);
            }
            Message::GuessRequest { client, rgb } => {
                let mut req = root.init_guess_req();

                let mut color = req.reborrow().init_rgb();

                color.set_r(rgb[0]);
                color.set_g(rgb[1]);
                color.set_b(rgb[2]);

                req.set_client(client);
            }
            Message::GuessResponse {
                client,
                rgb,
                closeness,
                closest,
            } => {
                let mut res = root.init_guess_res();

                let mut color = res.reborrow().init_rgb();

                color.set_r(rgb[0]);
                color.set_g(rgb[1]);
                color.set_b(rgb[2]);

                res.set_client(client);
                res.set_closeness(closeness);
                res.set_closest(closest);
            }
            _ => unimplemented!(),
        }

        let mut packed = Vec::new();

        serialize_packed::write_message(&mut packed, &serialized)?;
        // let mut people = root.init_people(2);
        //
        Ok(packed)
    }
}

pub type ClientID = String;

#[derive(Clone, Copy)]
enum Animation {
    Correct,
    TimeWarning,
    Timeout,
}

pub struct GameState {
    secret_color: [u8; 3],
    closest_color: [u8; 3],
    closest_client: Option<ClientID>,
    closeness: f32,
    threshold: f32,
    round_time: u8,
    animation_duration: u32,
    animation: Option<Animation>,
}

fn random_color() -> [u8; 3] {
    unsafe {
        [
            ((esp_random() % 16).pow(2)).min(u8::MAX.into()) as u8,
            ((esp_random() % 16).pow(2)).min(u8::MAX.into()) as u8,
            ((esp_random() % 16).pow(2)).min(u8::MAX.into()) as u8,
        ]
    }
}

const DIFF: f32 = 85.;

impl GameState {
    pub fn new() -> Self {
        Self {
            secret_color: [0, 0, 0],
            closest_color: [0, 0, 0],
            closest_client: None,
            closeness: 0.,
            threshold: DIFF,
            round_time: 0,
            animation: None,
            animation_duration: 0,
        }
    }

    fn round_config(&self) -> RoundConfig {
        let st: DateTime<Utc> = SystemTime::now().into();
        info!("round started at {}", st.format("%H:%M:%S"));

        RoundConfig {
            difficulty: self.threshold as u8,
            remaining: self.round_time,
            secret: self.secret_color,
            now: st,
            restart: false,
        }
    }

    pub fn restart(&mut self) -> Message {
        self.secret_color = random_color();
        self.closest_color = [0, 0, 0];
        self.closest_client = None;
        self.closeness = 0.;
        self.threshold = DIFF;
        self.round_time = 40;

        return Message::RoundConfig(RoundConfig {
            restart: true,
            ..self.round_config()
        });
    }

    pub fn recv(&mut self, msg: Message) -> Option<Message> {
        match msg {
            Message::Hello => {
                if self.round_time == 0 {
                    return Some(self.restart());
                }

                Some(Message::RoundConfig(self.round_config()))
            }
            Message::GuessRequest { client, rgb } => {
                let secret = colorutils::rgb8_to_oklab(&self.secret_color);
                let guess = colorutils::rgb8_to_oklab(&rgb);

                let c = colorutils::closeness(&secret, &guess);

                info!("went {rgb:?} on {:?} ({c}% close)", self.secret_color);
                info!(
                    "closest is {:?} with {:?} ({}% close)",
                    self.closest_client, self.closest_color, self.closeness
                );

                let is_closest = c >= self.closeness;

                if is_closest {
                    self.closest_client = Some(client.clone());
                    self.closest_color = rgb;
                    self.closeness = c;

                    if c >= self.threshold {
                        self.round_time = 5;
                        self.animation = Some(Animation::Correct);
                    }
                }

                Some(Message::GuessResponse {
                    client,
                    rgb: rgb.clone(),
                    closeness: c as u8,
                    closest: is_closest,
                })
                // Srgb::convert::<Hsl>(color.map(|c| (c as f32) / 256.))
            }
            Message::Tick => {
                let st: DateTime<Utc> = SystemTime::now().into();

                if self.round_time > 0 {
                    self.round_time -= 1;

                    if self.round_time < 5 {
                        self.animation = Some(Animation::TimeWarning);
                    }

                    if self.round_time == 0 {
                        return Some(self.restart());
                    }
                }

                None
            }
            _ => None,
        }
    }

    pub fn update_leds(
        &mut self,
        led1: &mut RGBDriver,
        led2: &mut RGBDriver,
    ) -> Result<(), EspError> {
        match self.animation {
            None => {
                led1.set(&self.closest_color)?;
                led2.set(&self.secret_color)?;
            }
            Some(animation) => {
                if self.animation_duration == 0 {
                    self.animation_duration = match animation {
                        Animation::Correct => 40,
                        Animation::TimeWarning => 4,
                        Animation::Timeout => 40,
                    }
                }

                match animation {
                    Animation::Correct => {
                        if self.animation_duration % 8 >= 4 {
                            led1.set(&[0, 255, 0])?;
                            led2.set(&[0, 255, 0])?;
                        } else {
                            led1.set(&[0, 0, 0])?;
                            led2.set(&[0, 0, 0])?;
                        }
                    }
                    Animation::TimeWarning => {
                        led1.set(&[255, 255, 0])?;
                        led2.set(&[255, 255, 0])?;
                    }
                    Animation::Timeout => {
                        if self.animation_duration % 8 >= 4 {
                            led1.set(&[255, 0, 255])?;
                            led2.set(&[255, 0, 255])?;
                        } else {
                            led1.set(&[0, 0, 0])?;
                            led2.set(&[0, 0, 0])?;
                        }
                    }
                }

                self.animation_duration -= 1;

                if self.animation_duration == 0 {
                    self.animation = None
                }
            }
        }
        Ok(())
    }
}

// ciiii...  c: command, i: client id, ...: display name
const HELLO: u8 = 1;
// ciiiirgb  c: command, i: client id, rgb: color
const GUESS_COLOR: u8 = 2;
// ciiiirgbd c: command, i: client id, rgb: color, d: distance
const GUESS_RESULT: u8 = 3;
// crstttttttt      c: command, r: remaining, s: restart, t: timestamp
const TICK: u8 = 6;
// cdTrgbtttttttt  c: command, d: difficulty, T: remaining, t: timestamp
const ROUND_START: u8 = 7;
// cdTrgbtttttttt  c: command, d: difficulty, T: remaining, t: timestamp
const ROUND_CONFIG: u8 = 8;

// impl TryFrom<&[u8]> for Message {
//     type Error = &'static str;

//     fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
//         let Some(client) = data.get(1..=4) else {
//             return Err("data is too short (in general)");
//         };

//         match data[0] {
//             HELLO => Ok(Self::Hello),
//             GUESS_COLOR => {
//                 if let Some(rgb) = data.get(5..=7) {
//                     Ok(Self::GuessRequest {
//                         client: client.try_into().unwrap(),
//                         rgb: rgb.try_into().unwrap(),
//                     })
//                 } else {
//                     Err("data is too short for GuessColor")
//                 }
//             }
//             _ => Err("unknown command"),
//         }
//     }
// }

// impl Event {
//     pub fn as_bytes(&self) -> Vec<u8, 16> {
//         let mut vec = Vec::new();

//         match self {
//             Self::RoundConfig(config) => {
//                 vec.push(ROUND_CONFIG).unwrap();
//                 vec.push(config.difficulty).unwrap();
//                 vec.push(config.remaining).unwrap();
//                 vec.extend(config.secret);
//                 vec.extend(config.now.timestamp().to_be_bytes());
//             }
//             Self::RoundStart(config) => {
//                 vec.push(ROUND_START).unwrap();
//                 vec.push(config.difficulty).unwrap();
//                 vec.push(config.remaining).unwrap();
//                 vec.extend(config.secret);
//                 vec.extend(config.now.timestamp().to_be_bytes());
//             }
//             &Self::GuessResult {
//                 client,
//                 rgb,
//                 closeness,
//                 closest,
//             } => {
//                 vec.push(GUESS_RESULT).unwrap();
//                 vec.extend(client);
//                 vec.extend(rgb);
//                 vec.push(closeness | (closest as u8) << 7).unwrap();
//             }
//             Self::Tick { remaining, now } => {
//                 vec.push(TICK).unwrap();
//                 vec.push(*remaining).unwrap();
//                 vec.extend(now.timestamp().to_be_bytes());
//             }
//         }

//         vec
//     }
// }
