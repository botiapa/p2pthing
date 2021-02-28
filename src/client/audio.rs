use std::{collections::HashMap, sync::{Arc, Mutex}};

use cpal::{Device, Host, SampleFormat, traits::{DeviceTrait, HostTrait, StreamTrait}};
use magnum_opus::{Bitrate, Channels, Decoder, Encoder};
use mio_misc::channel::Sender as MioSender;
use ringbuf::{Producer, RingBuffer};
use rubato::{FftFixedIn, InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction};

use crate::common::{debug_message::DebugMessageType, encryption::NetworkedPublicKey, message_type::InterthreadMessage};
use itertools::Itertools;

use super::tui::Tui;

pub struct Audio {
    host: Host,
    ui_s: MioSender<InterthreadMessage>,
    cm_s: MioSender<InterthreadMessage>,
    input_stream: Option<cpal::Stream>,
    output_stream: Option<cpal::Stream>,
    encoder: Arc<Mutex<Option<Encoder>>>,
    decoder: Option<Decoder>,
    muted: bool,
    input_resampler: Option<FftFixedIn<f32>>,
    input_channels: Option<usize>,
    output_resampler: Option<SincFixedIn<f32>>,
    out_s: Option<Producer<(NetworkedPublicKey, Vec<f32>)>>,
    preferred_kbits: Option<i32>,
    preferred_input_device: Option<Device>,
    preferred_output_device: Option<Device>,
    pub started: bool
}

impl Audio{
    pub fn new(ui_s: MioSender<InterthreadMessage>, cm_s: MioSender<InterthreadMessage>) -> Self {
        Audio {
            host: cpal::default_host(),
            ui_s,
            cm_s,
            input_stream: None,
            output_stream: None,
            encoder: Arc::new(Mutex::new(None)),
            decoder: None,
            muted: true,
            input_resampler: None,
            input_channels: None,
            output_resampler: None,
            out_s: None,
            preferred_kbits: None,
            preferred_input_device: None,
            preferred_output_device: None,
            started: false,
        }
    }

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
                if !self.muted { //FIXME
                    self.create_input_stream();
                }
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

    pub fn resample_and_send_packet(&mut self, packet: Vec<f32>) {
        let mut split = [Vec::with_capacity(packet.len()/2), Vec::with_capacity(packet.len()/2)];
        for ch in packet.chunks_exact(self.input_channels.unwrap()) {
            split[0].push(ch[0]);
            match self.input_channels.unwrap() {
                1 => split[1].push(ch[0]),
                2 => split[1].push(ch[1]),
                n => panic!("Invalid channel size: {}", n)
            }
        }
        
        let mut resampled = self.input_resampler.as_mut().unwrap().process(&split[..]).unwrap();
        let f_ch = resampled.pop().unwrap();
        let s_ch = resampled.pop().unwrap();

        let resampled = f_ch.into_iter().interleave(s_ch.into_iter()).collect::<Vec<f32>>();

        let mut output = [0u8; 2000];
        let mut lock = self.encoder.lock();
        let encoder = lock.as_mut().unwrap().as_mut().unwrap();
        let sent = encoder.encode_float(&resampled[..], &mut output).unwrap();

        self.cm_s.send(InterthreadMessage::OpusPacketReady(output[..sent].to_vec())).unwrap();
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

                let mut encoder = Encoder::new(48000, channels, magnum_opus::Application::Voip).unwrap();
                encoder.set_bitrate(bitrate).unwrap();

                let encoder = Arc::new(Mutex::new(Some(encoder)));
                self.encoder = Arc::clone(&encoder);

                let sample_rate = default_config.sample_rate.0;
                if sample_rate != 48000 {
                    self.input_resampler = Some(Audio::get_resampler_new(
                        sample_rate as usize, 
                        48000, 
                        channels as usize, 
                        sample_rate as usize / 50 as usize / channels as usize));
                    self.input_channels = Some(channels as usize);
                }

                self.input_stream = Some(match supported_default_config.sample_format() {
                    SampleFormat::I16 => unimplemented!("I16 input is not supported yet"),
                    SampleFormat::U16 => unimplemented!("U16 input is not supported yet"),
                    SampleFormat::F32 => 
                        device.build_input_stream(&default_config,
                move |data: &[f32], _: &_| {
                                //let max = data.iter().fold(0.0f32, |max, &val| if val > max{ val } else{ max });
                                //println!("Max: {}", max);
                                if sample_rate == 48000 {
                                    let mut output = [0u8; 2000];
                                    let mut lock = encoder.lock();
                                    let encoder = lock.as_mut().unwrap().as_mut().unwrap();
                                    let sent = encoder.encode_float(data, &mut output).unwrap();
    
                                    cm_s.send(InterthreadMessage::OpusPacketReady(output[0..sent].to_vec())).unwrap();
                                }
                                else {
                                    cm_s.send(InterthreadMessage::PacketReadyForResampling(data.to_vec())).unwrap();
                                }
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

                let (out_s, mut out_r) = RingBuffer::<(NetworkedPublicKey, Vec<f32>)>::new(20).split();
                self.out_s = Some(out_s);

                let output_samplerate = default_config.sample_rate.0;
                let channel_num = default_config.channels;

                if output_samplerate != 48000 {
                    let resampler = Audio::get_resampler(48000, default_config.sample_rate.0 as usize, channel_num as usize, 480);
                    self.output_resampler = Some(resampler);
                }

                self.output_stream = Some(device.build_output_stream(&default_config, 
                    move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            if !out_r.is_empty() {
                                //let start = Instant::now();
                                let mut queue = HashMap::new();
                                for (p, samples) in out_r.pop() {
                                    queue.insert(p, samples);
                                }
                                for i in 0..output.len() {
                                    let mut total = 0f32;
                                    for samples in queue.values() {
                                        if i < samples.len() {
                                            total += samples[i];
                                        }
                                    }
                                    output[i] = total / queue.values().len() as f32; // Average out the samples
                                }
                                //println!("Elapsed: {:?}", start.elapsed());
                            }
                            else {
                                for n in output {
                                    *n = 0.0;
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

    pub fn get_resampler_new(in_hz: usize, out_hz: usize, ch_in: usize, chunk_size: usize) -> FftFixedIn<f32> {
        FftFixedIn::<f32>::new(
             in_hz,
             out_hz,
            chunk_size,
            4,
            ch_in,
        )
    }

    pub fn decode_and_queue_packet(&mut self, data: &[u8], peer: NetworkedPublicKey) {
        let decoder = self.decoder.as_mut().unwrap();
        
        let channels = magnum_opus::packet::get_nb_channels(data).unwrap();
        let num_channels = match channels {
            Channels::Mono => 1,
            Channels::Stereo => 2
        };

        let mut out = [0f32; 5760*2];
        let read = decoder.decode_float(&data, &mut out, false).unwrap(); // reads 480 samples per channel

        let buf = self.out_s.as_mut().unwrap();
        match self.output_resampler.as_mut() {
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

                let resampled = f_ch.into_iter().interleave(s_ch.into_iter()).collect::<Vec<f32>>();
                if buf.push((peer, resampled)).is_err() {
                    panic!("Couldn't push new sample to ringbuffer (It's probably full)");
                }
            }
            None => {
                if buf.push((peer, out[..read*2].to_vec())).is_err() {
                    panic!("Couldn't push new sample to ringbuffer (It's probably full)");
                }
            }
        }
    }
}