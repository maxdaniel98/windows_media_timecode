use std::sync::atomic::AtomicBool;
use std::time::SystemTime;
use std::{
    io::{stdin, stdout, Write},
    sync::{
        atomic::{AtomicI32, AtomicUsize},
        Arc,
    },
    time::UNIX_EPOCH,
};

use gsmtc::{ManagerEvent::*, PlaybackStatus, SessionUpdateEvent::*};
use midir::{MidiOutput, MidiOutputPort};

fn send_position(
    conn_out: &mut midir::MidiOutputConnection,
    position: i32,
) -> Result<(), midir::SendError> {
    let hours: i32 = position / 3600000;
    let remaining_milliseconds: i32 = position % 3600000;

    let minutes: i32 = remaining_milliseconds / 60000;
    let remaining_milliseconds: i32 = remaining_milliseconds % 60000;

    let seconds: i32 = remaining_milliseconds / 1000;
    let remaining_milliseconds: i32 = remaining_milliseconds % 1000;

    let frames: i32 = remaining_milliseconds * 25 / 1000;

    // Ensure values are within BCD range
    let hours_bcd: u8 = hours as u8;
    let minutes_bcd: u8 = minutes as u8;
    let seconds_bcd: u8 = seconds as u8;
    let frames_bcd: u8 = frames as u8;

    /*
    rr = 00: 24 frames/s
    rr = 01: 25 frames/s
    rr = 10: 29.97 frames/s (SMPTE drop-frame timecode)
    rr = 11: 30 frames/s
    */

    let rr: u8 = 0b01; // 25 frames/s

    let hours_rate_bcd: u8 = rr << 5 | hours_bcd;

    conn_out.send(&[
        0xF0,
        0x7F,
        0x7F,
        0x01,
        0x01,
        hours_rate_bcd,
        minutes_bcd,
        seconds_bcd,
        frames_bcd,
        0xF7,
    ])
}

fn send_mtc_quarter_frame(
    conn_out: &mut midir::MidiOutputConnection,
    position: i32,
    message_index: u8,
) -> Result<(), midir::SendError> {
    let hours: i32 = position / 3600000;
    let remaining_milliseconds: i32 = position % 3600000;

    let minutes: i32 = remaining_milliseconds / 60000;
    let remaining_milliseconds: i32 = remaining_milliseconds % 60000;

    let seconds: i32 = remaining_milliseconds / 1000;
    let remaining_milliseconds: i32 = remaining_milliseconds % 1000;

    let frames: i32 = remaining_milliseconds * 25 / 1000;

    let frames_low_nibble: u8 = (frames & 0x0F) as u8;
    let frames_high_nibble: u8 = ((frames >> 4) & 0x01) as u8;

    let seconds_low_nibble: u8 = (seconds & 0x0F) as u8;
    let seconds_high_nibble: u8 = ((seconds >> 4) & 0x03) as u8;

    let minutes_low_nibble: u8 = (minutes & 0x0F) as u8;
    let minutes_high_nibble: u8 = ((minutes >> 4) & 0x03) as u8;

    let hours_low_nibble: u8 = (hours & 0x0F) as u8;
    let rate: u8 = 0b01; // 25 frames/s
    let hours_high_nibble: u8 = ((hours >> 4) & 0x01) as u8 | (rate << 1);

    let quarter_frames = [
        0xF1,
        frames_low_nibble,
        0xF1,
        frames_high_nibble | 0x10,
        0xF1,
        seconds_low_nibble | 0x20,
        0xF1,
        seconds_high_nibble | 0x30,
        0xF1,
        minutes_low_nibble | 0x40,
        0xF1,
        minutes_high_nibble | 0x50,
        0xF1,
        hours_low_nibble | 0x60,
        0xF1,
        hours_high_nibble | 0x70,
    ];

    let messages = quarter_frames.chunks(2);

    // Send only the requested quarter frame (by message_index)
    let msg = messages
        .skip(message_index as usize)
        .take(1)
        .next()
        .unwrap();

    conn_out.send(msg)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let play_position = Arc::new(AtomicI32::new(0));
    let last_play_position_update = Arc::new(AtomicUsize::new(0));
    let last_sent_position_update = Arc::new(AtomicUsize::new(0));
    let is_playing = Arc::new(AtomicBool::new(false));

    // read config file from argument (if provided) or use default
    let config_file = std::env::args().nth(1).unwrap_or("config.json".to_string());

    // read file
    let config = std::fs::read_to_string(config_file).expect("error reading config file");

    // parse config
    let config: serde_json::Value = serde_json::from_str(&config)?;

    println!("Config: {:#?}", config);

    let midi_out = MidiOutput::new("Timecode")?;

    // Get an output port (read from console if multiple are available)
    let out_ports = midi_out.ports();
    let out_port: &MidiOutputPort = match out_ports.len() {
        0 => return Err("no output port found".into()),
        1 => {
            println!(
                "Choosing the only available output port: {}",
                midi_out.port_name(&out_ports[0]).unwrap()
            );
            &out_ports[0]
        }
        _ => {
            println!("\nAvailable output ports:");
            for (i, p) in out_ports.iter().enumerate() {
                println!("{}: {}", i, midi_out.port_name(p).unwrap());
            }
            print!("Please select output port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            out_ports
                .get(input.trim().parse::<usize>()?)
                .ok_or("invalid output port selected")?
        }
    };

    let mut conn_out = midi_out.connect(out_port, "Timecode")?;

    // Send timecode message of time 00:00:00:00
    send_position(&mut conn_out, 0)?;

    // async send timecode message every second
    let cloned_play_position = play_position.clone();
    let cloned_last_play_position_update = last_play_position_update.clone();
    let cloned_last_sent_position_update = last_sent_position_update.clone();
    let cloned_is_playing = is_playing.clone();

    tokio::spawn(async move {
        let mut message_index = 0;

        loop {
            let mut position =
                cloned_play_position.load(std::sync::atomic::Ordering::Relaxed) as i32;
            let last_update =
                cloned_last_play_position_update.load(std::sync::atomic::Ordering::Relaxed);
            let last_sent_update =
                cloned_last_sent_position_update.load(std::sync::atomic::Ordering::Relaxed);
            let is_playing = cloned_is_playing.load(std::sync::atomic::Ordering::Relaxed);

            let now = SystemTime::now();
            let now: std::time::Duration =
                now.duration_since(UNIX_EPOCH).expect("Time went backwards");

            let elapsed = now
                .as_millis()
                .checked_sub(last_update as u128)
                .unwrap_or(0);

            if (is_playing) {
                position = position + elapsed as i32;
            }

            if (last_update != last_sent_update) {
                send_position(&mut conn_out, position).unwrap();
                cloned_last_sent_position_update
                    .store(last_update, std::sync::atomic::Ordering::Relaxed);
            }

            if (is_playing) {
                send_mtc_quarter_frame(&mut conn_out, position, message_index).unwrap();
            }

            message_index = (message_index + 1) % 8;

            tokio::time::sleep(tokio::time::Duration::from_millis(1000 / 25 / 8)).await;
        }
    });

    let mut rx = gsmtc::SessionManager::create().await?;

    while let Some(evt) = rx.recv().await {
        match evt {
            SessionCreated {
                session_id,
                mut rx,
                source,
            } => {
                println!("Created session: {{id={session_id}, source={source}}}");

                let play_position = play_position.clone();
                let last_play_position_update = last_play_position_update.clone();
                let is_playing = is_playing.clone();

                tokio::spawn(async move {
                    while let Some(evt) = rx.recv().await {
                        match evt {
                            Model(mut model) => {
                                let timeline = model.timeline.as_mut();
                                timeline.map(|timeline| {
                                    let position: i32 = (timeline.position / 10000) as i32;
                                    play_position
                                        .store(position, std::sync::atomic::Ordering::Relaxed);

                                    let updated_at: usize = timeline.last_updated_at_ms as usize;

                                    last_play_position_update
                                        .store(updated_at, std::sync::atomic::Ordering::Relaxed);
                                });
                                let playback = model.playback.as_mut();
                                playback.map(|playback| {
                                    is_playing.store(
                                        playback.status == PlaybackStatus::Playing,
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                });

                                println!("[{session_id}/{source}] Model updated: {model:#?}")
                            }
                            Media(model, image) => println!(
                                "[{session_id}/{source}] Media updated: {model:#?} - {image:?}"
                            ),
                        }
                    }
                    println!("[{session_id}/{source}] exited event-loop");
                });
            }
            SessionRemoved { session_id } => println!("Session {{id={session_id}}} was removed"),
            CurrentSessionChanged {
                session_id: Some(id),
            } => println!("Current session: {id}"),
            CurrentSessionChanged { session_id: None } => println!("No more current session"),
        }
    }

    Ok(())
}
