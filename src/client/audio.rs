use std::sync::{Arc, Mutex};

use cpal::{Device, Host, Sample, SampleFormat, traits::{DeviceTrait, HostTrait, StreamTrait}};
use magnum_opus::{Bitrate, Channels, Decoder, Encoder};
use mio_misc::channel::Sender;
use ringbuf::{Producer, RingBuffer};
use rubato::{InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction};

use crate::common::{debug_message::DebugMessageType, message_type::InterthreadMessage};
use itertools::Itertools;

use super::tui::Tui;

pub struct Audio {
    host: Host,
    ui_s: Sender<InterthreadMessage>,
    cm_s: Sender<InterthreadMessage>,
    input_stream: Option<cpal::Stream>,
    output_stream: Option<cpal::Stream>,
    encoder: Arc<Mutex<Option<Encoder>>>,
    decoder: Option<Decoder>,
    muted: bool,
    resampler: Option<SincFixedIn<f32>>,
    buffer: Option<Producer<f32>>,
    preferred_kbits: Option<i32>,
    preferred_input_device: Option<Device>,
    preferred_output_device: Option<Device>,
    pub started: bool
}

impl Audio{
    pub fn new(ui_s: Sender<InterthreadMessage>, cm_s: Sender<InterthreadMessage>) -> Self {
        Audio {
            host: cpal::default_host(),
            ui_s,
            cm_s,
            input_stream: None,
            output_stream: None,
            encoder: Arc::new(Mutex::new(None)),
            decoder: None,
            muted: true,
            resampler: None,
            buffer: None,
            preferred_kbits: None,
            preferred_input_device: None,
            preferred_output_device: None,
            started: false,
        }
    }

    //TODO: Split this into multiple functions; therefore disabling automatic recording
    //TODO: Also rename this to init or something
    pub fn init(&mut self) {
        self.create_output_stream();
        if !self.muted {
            self.create_input_stream();
        }
        self.update_input_devices();
        self.update_output_devices();

        self.started = true;
    }

    pub fn change_input_device(&mut self, name: String) {
        match &mut self.host.input_devices() {
            Ok(devices) => {
                let d = devices.find(|d| d.name().unwrap() == name).unwrap();
                self.preferred_input_device = Some(d);
                self.create_input_stream();
            }
            _ => {}
        }
    }

    pub fn change_preferred_kbits(&mut self, kbits: i32) {
        self.preferred_kbits = Some(kbits);
        self.create_input_stream();
    }

    pub fn change_output_device(&mut self, name: String) {
        match &mut self.host.output_devices() {
            Ok(devices) => {
                let d = devices.find(|d| d.name().unwrap() == name).unwrap();
                self.preferred_output_device = Some(d);
                self.create_output_stream();
            }
            _ => {}
        }
    }

    pub fn change_mute_state(&mut self, muted: bool) {
        match muted {
            true => self.input_stream = None,
            false => self.create_input_stream()
        }
    }

    pub fn update_input_devices(&mut self) {
        match self.host.input_devices() {
            Ok(devices) => {
                let devices: Vec<String> = devices.map(|d| d.name().unwrap()).collect();
                match devices.len() {
                    c if c > 0 => self.ui_s.send(InterthreadMessage::AudioNewInputDevices(Some(devices))).unwrap(),
                    _ => self.ui_s.send(InterthreadMessage::AudioNewInputDevices(None)).unwrap()
                }
            },
            Err(err) => {
                self.ui_s.send(InterthreadMessage::AudioNewInputDevices(None)).unwrap();
                Tui::debug_message(&format!("Error getting input devices: {}", err), DebugMessageType::Warning, &self.ui_s);
            }
        }
    }

    pub fn update_output_devices(&mut self) {
        match self.host.output_devices() {
            Ok(devices) => {
                let devices: Vec<String> = devices.map(|d| d.name().unwrap()).collect();
                match devices.len() {
                    c if c > 0 => self.ui_s.send(InterthreadMessage::AudioNewOutputDevices(Some(devices))).unwrap(),
                    _ => self.ui_s.send(InterthreadMessage::AudioNewOutputDevices(None)).unwrap()
                }
            },
            Err(err) => {
                self.ui_s.send(InterthreadMessage::AudioNewOutputDevices(None)).unwrap();
                Tui::debug_message(&format!("Error getting output devices: {}", err), DebugMessageType::Warning, &self.ui_s);
            }
        }
    }

    fn create_input_stream(&mut self) {
        let default_input = &self.host.default_input_device();
        let device = match &self.preferred_input_device {
            Some(d) => d,
            None => 
                match default_input {
                    Some(d) => d,
                    None => {
                        Tui::debug_message("Cannot find default input device", DebugMessageType::Error, &self.ui_s);
                        return;
                    }
                }
        };

        let bitrate = match self.preferred_kbits {
            Some(kbits) => Bitrate::Bits(kbits * 1000 * 8),
            None => Bitrate::Bits(128 * 1000 * 8)
        };

        let default_config = device.default_input_config();
        match default_config {
            Ok(supported_default_config) => {
                let cm_s = self.cm_s.clone();
                let ui_s = self.ui_s.clone();

                let default_config = supported_default_config.config();

                let channels = match default_config.channels {
                    1 => Channels::Mono,
                    2 => Channels::Stereo,
                    c => panic!("Invalid channels detected: {}", c)
                };

                let mut encoder = Encoder::new(default_config.sample_rate.0, channels, magnum_opus::Application::Voip).unwrap();
                encoder.set_bitrate(bitrate).unwrap();

                let encoder = Arc::new(Mutex::new(Some(encoder)));
                self.encoder = Arc::clone(&encoder);

                self.input_stream = Some(match supported_default_config.sample_format() {
                    SampleFormat::I16 => unimplemented!("I16 input is not supported yet"),
                    SampleFormat::U16 => unimplemented!("U16 input is not supported yet"),
                    SampleFormat::F32 => 
                        device.build_input_stream(&default_config,
                move |data: &[f32], _: &_| {
                                let mut output = [0u8; 2000];
                                let mut lock = encoder.lock();
                                let encoder = lock.as_mut().unwrap().as_mut().unwrap();
                                let sent = encoder.encode_float(data, &mut output).unwrap();

                                cm_s.send(InterthreadMessage::OpusPacketReady(output[0..sent].to_vec())).unwrap();
                            },
                            move |err| {
                                Tui::debug_message(&format!("Read error from input err: {}", err), DebugMessageType::Error, &ui_s);
                            },
                        ).unwrap(),
                });
                self.input_stream.as_ref().unwrap().play().unwrap();
                Tui::debug_message(&format!("Recording started: device: {} sample_rate: {} channels: {}", device.name().unwrap(), default_config.sample_rate.0, default_config.channels), DebugMessageType::Log, &self.ui_s);
            }
            Err(err) => Tui::debug_message(&format!("Cannot find config for default input device err: {}", err), DebugMessageType::Error, &self.ui_s)
        }
   
    }

    fn create_output_stream(&mut self) {
        let default_output = &self.host.default_output_device();
        let device = match &self.preferred_output_device {
            Some(d) => d,
            None => 
                match default_output {
                    Some(d) => d,
                    None => {
                        Tui::debug_message("Cannot find default output device", DebugMessageType::Error, &self.ui_s);
                        return;
                    }
                }
        };
        
        let default_config = device.default_output_config();
        match default_config {
            Ok(supported_default_config) => {
                let default_config = supported_default_config.config();

                let channels = match default_config.channels {
                    1 => Channels::Mono,
                    2 => Channels::Stereo,
                    c => panic!("Invalid channels detected: {}", c)
                };
                let decoder = Decoder::new(48000, channels).unwrap();
                self.decoder = Some(decoder);

                let (prod, mut cons) = RingBuffer::<f32>::new(5760*2).split();
                self.buffer = Some(prod);

                let output_samplerate = default_config.sample_rate.0;
                let channel_num = default_config.channels;

                if output_samplerate != 48000 {
                    let resampler = Audio::get_resampler(48000, default_config.sample_rate.0 as usize, channel_num as usize, 480);
                    self.resampler = Some(resampler);
                }

                self.output_stream = Some(device.build_output_stream(&default_config, 
                    move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            // This is not optimized, however it's fast enough (2µs on average), and it avoids a beeping noise
                            // TODO: Optimize me
                            for i in 0..output.len() {
                                match cons.pop() {
                                    Some(item) => output[i] = Sample::from::<f32>(&item),
                                    None => output[i] = Sample::from::<f32>(&0.0)
                                }
                            }
                        }
                        , move |err| {
                        panic!(err);
                    }).unwrap());
                self.output_stream.as_ref().unwrap().play().unwrap();
                Tui::debug_message(&format!("Playback started: device: {} sample_rate: {} channels: {}", device.name().unwrap(), default_config.sample_rate.0, default_config.channels), DebugMessageType::Log, &self.ui_s);
            }
            Err(err) => {
                Tui::debug_message(&format!("Cannot find config for default output device err: {}", err), DebugMessageType::Error, &self.ui_s);
                panic!("Cannot find config for default output device err: {}", err);
            }
        }
    }

    pub fn get_resampler(in_hz: usize, out_hz: usize, ch_in: usize, chunk_size: usize) -> SincFixedIn<f32> {
        let sinc_len = 256;
        let f_cutoff = 0.925914648491266;
        let params = InterpolationParameters {
            sinc_len,
            f_cutoff,
            interpolation: InterpolationType::Linear,
            oversampling_factor: 2048,
            window: WindowFunction::Blackman2,
        };
        SincFixedIn::<f32>::new(
            out_hz as f64 / in_hz  as f64,
            params,
            chunk_size,
            ch_in,
        )
    }

    pub fn decode_and_queue_packet(&mut self, data: &[u8]) {
        let decoder = self.decoder.as_mut().unwrap();
        
        let channels = magnum_opus::packet::get_nb_channels(data).unwrap();
        let num_channels = match channels {
            Channels::Mono => 1,
            Channels::Stereo => 2
        };

        let mut out = [0f32; 5760*2];
        let read = decoder.decode_float(&data, &mut out, false).unwrap(); // reads 480 samples per channel

        let buf = self.buffer.as_mut().unwrap();
        match self.resampler.as_mut() {
            Some(resampler) => {
                let mut split = [Vec::with_capacity(read), Vec::with_capacity(read)];
                for ch in out[..read*num_channels].chunks_exact(num_channels) {
                    split[0].push(ch[0]);
                    match channels {
                        Channels::Mono => split[1].push(ch[0]),
                        Channels::Stereo => split[1].push(ch[1])
                    }
                }
                let mut resampled = resampler.process(&split).unwrap();
                let f_ch = resampled.pop().unwrap();
                let s_ch = resampled.pop().unwrap();

                let mut resampled = f_ch.into_iter().interleave(s_ch.into_iter());
                buf.push_iter(&mut resampled);
            }
            None => {
                buf.push_slice(&out[..read*2]);
            }
        }
    }
}