use std::collections::HashMap;

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, SampleFormat,
};
use magnum_opus::{Bitrate, Channels, Decoder, Encoder};
use p2pthing_common::mio_misc::channel::Sender as MioSender;
use p2pthing_common::{encryption::NetworkedPublicKey, message_type::InterthreadMessage, ui::UIConn};
//use nnnoiseless::DenoiseState;
use ringbuf::{Producer, RingBuffer};
use rubato::{FftFixedIn, InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction};

use itertools::Itertools;

pub struct Audio {
    host: Host,
    ui_s: MioSender<InterthreadMessage>,
    cm_s: MioSender<InterthreadMessage>,
    input_stream: Option<cpal::Stream>,
    output_stream: Option<cpal::Stream>,
    encoder: Option<Encoder>,
    decoder: Option<Decoder>,
    //denoiser: Option<(Box<DenoiseState<'static>>, Box<DenoiseState<'static>>)>,
    muted: bool,
    input_resampler: Option<FftFixedIn<f32>>,
    input_channels: Option<usize>,
    output_resampler: Option<SincFixedIn<f32>>,
    out_s: Option<Producer<(NetworkedPublicKey, Vec<f32>)>>,
    preferred_kbits: Option<i32>,
    preferred_input_device: Option<Device>,
    preferred_output_device: Option<Device>,
    pub started: bool,
}

impl Audio {
    pub fn new(ui_s: MioSender<InterthreadMessage>, cm_s: MioSender<InterthreadMessage>) -> Self {
        Audio {
            host: cpal::default_host(),
            ui_s,
            cm_s,
            input_stream: None,
            output_stream: None,
            encoder: None,
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
            //denoiser: None,
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
                if !self.muted {
                    //FIXME
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
            false => self.create_input_stream(),
        }
    }

    /*pub fn change_denoiser_state(&mut self, denoiser_state: bool) {
        if denoiser_state && self.denoiser.is_some() {return;}
        else {
            self.denoiser = match denoiser_state {
                true => Some((DenoiseState::new(), DenoiseState::new())),
                false => None
            }
        }

    }*/

    pub fn update_input_devices(&mut self) {
        match self.host.input_devices() {
            Ok(devices) => {
                let devices: Vec<String> = devices.map(|d| d.name().unwrap()).collect();
                match devices.len() {
                    c if c > 0 => self.ui_s.send(InterthreadMessage::AudioNewInputDevices(Some(devices))).unwrap(),
                    _ => self.ui_s.send(InterthreadMessage::AudioNewInputDevices(None)).unwrap(),
                }
            }
            Err(err) => {
                self.ui_s.send(InterthreadMessage::AudioNewInputDevices(None)).unwrap();
                self.ui_s.log_warning(&format!("Error getting input devices: {}", err));
            }
        }
    }

    pub fn update_output_devices(&mut self) {
        match self.host.output_devices() {
            Ok(devices) => {
                let devices: Vec<String> = devices.map(|d| d.name().unwrap()).collect();
                match devices.len() {
                    c if c > 0 => self.ui_s.send(InterthreadMessage::AudioNewOutputDevices(Some(devices))).unwrap(),
                    _ => self.ui_s.send(InterthreadMessage::AudioNewOutputDevices(None)).unwrap(),
                }
            }
            Err(err) => {
                self.ui_s.send(InterthreadMessage::AudioNewOutputDevices(None)).unwrap();
                self.ui_s.log_warning(&format!("Error getting output devices: {}", err));
            }
        }
    }

    /// Resample the interleaved data with the provided resampler
    pub fn resample(resampler: &mut dyn Resampler<f32>, input: &[f32], ch_nmb: usize) -> Vec<f32> {
        let mut split = [Vec::with_capacity(input.len() / 2), Vec::with_capacity(input.len() / 2)];

        // Split the two channels
        for ch in input.chunks_exact(ch_nmb) {
            split[0].push(ch[0]);
            match ch_nmb {
                1 => split[1].push(ch[0]),
                2 => split[1].push(ch[1]),
                n => panic!("Invalid channel size: {}", n),
            }
        }

        let mut resampled = resampler.process(&split[..]).unwrap();
        let f_ch = resampled.pop().unwrap();
        let s_ch = resampled.pop().unwrap();

        f_ch.into_iter().interleave(s_ch.into_iter()).collect::<Vec<f32>>()
    }

    /// Apply denoising
    /*pub fn denoise(denoiser: &mut (Box<DenoiseState>, Box<DenoiseState>), input: &mut [f32], ch_nmb: usize) -> Vec<f32> {
        assert_eq!(input.len() % DenoiseState::FRAME_SIZE, 0, "Not optional input size in denoiser: input: {}  denoiser_frame_size: {}", input.len(), DenoiseState::FRAME_SIZE);

        // Convert from f32 to i16
        for n in input.iter_mut() {
            *n = match *n {
                n if n > 1.0f32 => i16::MAX as f32,
                n if n < -1.0f32 => i16::MIN as f32,
                _ => *n * i16::MAX as f32
            };
        }

        // Split the two channels
        let mut split = [Vec::with_capacity(input.len()/2), Vec::with_capacity(input.len()/2)];
        for ch in input.chunks_exact(ch_nmb) {
            split[0].push(ch[0]);
            match ch_nmb {
                1 => split[1].push(ch[0]),
                2 => split[1].push(ch[1]),
                n => panic!("Invalid channel size: {}", n)
            }
        }

        let (d1, d2) = denoiser;

        let mut output = Vec::with_capacity(input.len()/2);
        let mut out_buf = [0.0f32; DenoiseState::FRAME_SIZE];
        for chunk in split[0].chunks_exact(DenoiseState::FRAME_SIZE) {
            d1.process_frame(&mut out_buf, chunk);
            output.extend_from_slice(&out_buf[..]);
        }
        split[0] = output;

        let mut output = Vec::with_capacity(input.len()/2);
        let mut out_buf = [0.0f32; DenoiseState::FRAME_SIZE];
        for chunk in split[1].chunks_exact(DenoiseState::FRAME_SIZE) {
            d2.process_frame(&mut out_buf, chunk);
            output.extend_from_slice(&out_buf[..]);
        }
        split[1] = output;

        let f_ch = split[0].clone();
        let s_ch = split[1].clone();

        let mut output = f_ch.into_iter().interleave(s_ch.into_iter()).collect::<Vec<f32>>();

        // Convert back from i16 to f32
        for n in &mut output {
            *n = *n / i16::MAX as f32;
        }
        output
    }*/

    pub fn process_and_send_packet(&mut self, packet: Vec<f32>) {
        let mut resampled = match &mut self.input_resampler {
            Some(resampler) => Audio::resample(resampler, &packet[..], self.input_channels.unwrap()),
            None => packet,
        };

        /*let denoised = match &mut self.denoiser {
            Some(denoiser) => Audio::denoise(denoiser, &mut resampled[..], self.input_channels.unwrap()),
            None => resampled
        };*/
        let denoised = resampled;

        let mut output = [0u8; 2000];
        let encoder = self.encoder.as_mut().unwrap();
        let sent = encoder.encode_float(&denoised[..], &mut output).unwrap();

        self.cm_s.send(InterthreadMessage::OpusPacketReady(output[..sent].to_vec())).unwrap();
    }

    fn create_input_stream(&mut self) {
        let default_input = &self.host.default_input_device();
        let device = match &self.preferred_input_device {
            Some(d) => d,
            None => match default_input {
                Some(d) => d,
                None => {
                    self.ui_s.log_error("Cannot find default input device");
                    return;
                }
            },
        };

        let bitrate = match self.preferred_kbits {
            Some(kbits) => Bitrate::Bits(kbits * 1000 * 8),
            None => Bitrate::Bits(128 * 1000 * 8),
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
                    c => panic!("Invalid channels detected: {}", c),
                };

                let mut encoder = Encoder::new(48000, channels, magnum_opus::Application::Voip).unwrap();
                encoder.set_vbr(true).unwrap();
                encoder.set_bitrate(bitrate).unwrap();

                self.encoder = Some(encoder);

                //self.denoiser = Some((DenoiseState::new(), DenoiseState::new()));

                let sample_rate = default_config.sample_rate.0;
                self.input_channels = Some(channels as usize);
                if sample_rate != 48000 {
                    self.input_resampler = Some(Audio::get_resampler_new(
                        sample_rate as usize,
                        48000,
                        channels as usize,
                        sample_rate as usize / 50 as usize / channels as usize,
                    ));
                }

                self.input_stream = Some(match supported_default_config.sample_format() {
                    SampleFormat::I16 => unimplemented!("I16 input is not supported yet"),
                    SampleFormat::U16 => unimplemented!("U16 input is not supported yet"),
                    SampleFormat::F32 => device
                        .build_input_stream(
                            &default_config,
                            move |data: &[f32], _: &_| {
                                cm_s.send(InterthreadMessage::AudioDataReadyToBeProcessed(data.to_vec())).unwrap();
                            },
                            move |err| {
                                ui_s.log_error(&format!("Read error from input err: {}", err));
                            },
                        )
                        .unwrap(),
                });
                self.input_stream.as_ref().unwrap().play().unwrap();
                self.ui_s.log_info(&format!(
                    "Recording started: device: {} sample_rate: {} channels: {}",
                    device.name().unwrap(),
                    default_config.sample_rate.0,
                    default_config.channels
                ));
            }
            Err(err) => self.ui_s.log_info(&format!("Cannot find config for default input device err: {}", err)),
        }
    }

    fn create_output_stream(&mut self) {
        let default_output = &self.host.default_output_device();
        let device = match &self.preferred_output_device {
            Some(d) => d,
            None => match default_output {
                Some(d) => d,
                None => {
                    self.ui_s.log_error("Cannot find default output device");
                    return;
                }
            },
        };

        let default_config = device.default_output_config();
        match default_config {
            Ok(supported_default_config) => {
                let default_config = supported_default_config.config();

                let channels = match default_config.channels {
                    1 => Channels::Mono,
                    2 => Channels::Stereo,
                    c => panic!("Invalid channels detected: {}", c),
                };
                let decoder = Decoder::new(48000, channels).unwrap();
                self.decoder = Some(decoder);

                let (out_s, mut out_r) = RingBuffer::<(NetworkedPublicKey, Vec<f32>)>::new(100).split(); // Need a larger capacity due to how UDP packets are sent through the network
                self.out_s = Some(out_s);

                let output_samplerate = default_config.sample_rate.0;
                let channel_num = default_config.channels;

                if output_samplerate != 48000 {
                    let resampler =
                        Audio::get_resampler(48000, default_config.sample_rate.0 as usize, channel_num as usize, 480);
                    self.output_resampler = Some(resampler);
                }

                let ui_s = self.ui_s.clone();
                self.output_stream = Some(
                    device
                        .build_output_stream(
                            &default_config,
                            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                                if !out_r.is_empty() {
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
                                        output[i] = total / queue.values().len() as f32;
                                        // Average out the samples
                                    }
                                } else {
                                    for n in output {
                                        *n = 0.0;
                                    }
                                }
                            },
                            move |err| {
                                ui_s.log_error(&format!("Error while running output stream: {}", err));
                            },
                        )
                        .unwrap(),
                );
                self.output_stream.as_ref().unwrap().play().unwrap();
                self.ui_s.log_info(&format!(
                    "Playback started: device: {} sample_rate: {} channels: {}",
                    device.name().unwrap(),
                    default_config.sample_rate.0,
                    default_config.channels
                ))
            }
            Err(err) => {
                self.ui_s.log_error(&format!("Cannot find config for default output device err: {}", err));
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
        SincFixedIn::<f32>::new(out_hz as f64 / in_hz as f64, params, chunk_size, ch_in)
    }

    pub fn get_resampler_new(in_hz: usize, out_hz: usize, ch_in: usize, chunk_size: usize) -> FftFixedIn<f32> {
        FftFixedIn::<f32>::new(in_hz, out_hz, chunk_size, 4, ch_in)
    }

    pub fn decode_and_queue_packet(&mut self, data: &[u8], peer: NetworkedPublicKey) {
        let decoder = self.decoder.as_mut().unwrap();

        let channels = magnum_opus::packet::get_nb_channels(data).unwrap();
        let num_channels = match channels {
            Channels::Mono => 1,
            Channels::Stereo => 2,
        };

        let mut out = [0f32; 960]; // Was 5760*2
        let read = decoder.decode_float(&data, &mut out, false).unwrap(); // reads 480 samples per channel

        let buf = self.out_s.as_mut().unwrap();

        // Resample if needed
        let resampled = match self.output_resampler.as_mut() {
            Some(resampler) => Audio::resample(resampler, &out[..read * num_channels], num_channels),
            None => out[..read * num_channels].to_vec(),
        };
        if buf.push((peer, resampled)).is_err() {
            self.ui_s.log_error("Couldn't push new sample to ringbuffer (It's probably full)");
        }
    }
}
