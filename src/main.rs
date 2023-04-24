use std::env;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use hound::{WavWriter, WavSpec, SampleFormat};
use std::sync::atomic::{AtomicBool, Ordering};


fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    // Set up audio output stream
    let host = cpal::default_host();
    let device = host.default_output_device().expect("no input device available");

    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let (sender, receiver) = mpsc::channel();

    let err_fn = move |err| eprintln!("an error occurred on the output audio stream: {}", err);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                sender.send(data.to_vec()).unwrap();
            },
            err_fn,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let float_data: Vec<f32> = data.iter().map(|&x| x as f32 / i16::MAX as f32).collect();
                sender.send(float_data).unwrap();
            },
            err_fn,
        ),
        _ => panic!("unsupported sample format"),
    }.unwrap();

    stream.play().unwrap();

    // Start recording
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buffer_clone = buffer.clone();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();
    let handle = thread::spawn(move || {
        loop {
            if stop_flag_clone.load(Ordering::SeqCst) {
                break;
            }
            if let Ok(data) = receiver.recv() {
                let mut buffer = buffer_clone.lock().unwrap();
                buffer.extend_from_slice(&data);
            } else {
                println!("Error acquiring lock on buffer");
            }
        }
    });

    // Wait for user input to stop recording
    println!("Recording started. Press any key to stop recording...");
    loop {
        if poll(std::time::Duration::from_millis(500)).unwrap() {
            if let Event::Key(KeyEvent { code: KeyCode::Char(_), .. }) = read().unwrap() {
                stop_flag.store(true, Ordering::SeqCst);
                break;
            }
        }
        else { println!("recording...");}
    }

    println!("Finished recording.");
    // Stop recording
    let mut new_buffer: Option<Arc<Mutex<Vec<f32>>>> = None;
    let mut buffer: Arc<Mutex<Vec<f32>>> = loop {
        
        let locked_buffer = buffer.lock().unwrap();
        match Arc::try_unwrap(Arc::new(locked_buffer)) {
            Ok(inner) => match inner.clone().into_boxed_slice() {
                data => {
                    new_buffer = Some(Arc::new(Mutex::new(data.into_vec())));
                    break new_buffer.as_ref().unwrap().clone();
                }
            },
            Err(_arc) => {
                new_buffer = None;
                eprintln!("Error acquiring lock on buffer, retrying...");
                
            }
        }
    };
    
    if let Some(nb) = new_buffer {
        buffer = nb;
    }
    
    
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create("output.wav", spec).unwrap();
    for sample in (*(*buffer).lock().unwrap()).iter() {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
    // drop any data that is being held by the buffer

    

    handle.join().unwrap();
    println!("test.");
    
}
