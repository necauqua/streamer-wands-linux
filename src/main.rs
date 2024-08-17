use std::{
    path::PathBuf,
    sync::{mpsc::Receiver, Arc, Mutex},
    thread::{self, sleep},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use lazy_regex::lazy_regex;
use steamlocate::SteamDir;
use tungstenite::Message;

fn poll_file(file: PathBuf) -> Result<Receiver<String>> {
    let (messages_tx, messages_rx) = std::sync::mpsc::channel();

    let mut last = String::new();
    thread::spawn(move || loop {
        let Ok(data) = std::fs::read_to_string(&file) else {
            break;
        };
        if data != last {
            messages_tx.send(data.clone()).unwrap();
        }
        last = data;
        sleep(Duration::from_secs(1));
    });

    Ok(messages_rx)
}

fn send_loop(ws_url: &str, msg_rx: &Receiver<String>) -> Result<&'static str> {
    let (socket, response) = tungstenite::connect(ws_url)?;

    let s = response.status();
    if !s.is_success() && !s.is_informational() {
        if s.is_client_error() {
            return Ok("{s} response from the server, bad token? Try to re-auth");
        } else {
            bail!("{s} response from the server, is it down?.");
        }
    }

    // bruh I cant be bothered to setup better concurrency
    let socket = Arc::new(Mutex::new(socket));

    thread::spawn({
        let socket = socket.clone();
        move || {
            loop {
                sleep(Duration::from_secs(5)); // literally what the streamer wands mod does, idk
                socket
                    .lock()
                    .unwrap()
                    .send(Message::Text("im alive".to_owned()))
                    .unwrap();
            }
        }
    });

    let mut counter = 0;
    eprintln!("sent messages: {counter}");
    loop {
        socket
            .lock()
            .unwrap()
            .send(Message::Text(msg_rx.recv().unwrap()))?;
        msg_rx.recv().unwrap();
        counter += 1;

        eprintln!("\x1b[Fsent messages: {counter}");
    }
}

fn main() -> Result<()> {
    let mut steam = SteamDir::locate().context("Steam not found")?;
    let noita_dir = steam.app(&881100).context("Noita not found")?;

    let host_path = noita_dir.path.join("mods/streamer_wands/files/ws/host.lua");
    let token_path = noita_dir.path.join("mods/streamer_wands/token.lua");

    let host = std::fs::read_to_string(host_path)
        .context("Failed to read streamer wands host, is the mod installed?")?;
    let (_, [host]) = lazy_regex!("HOST_URL = \"(.*?)\"")
        .captures(&host)
        .context("Malformed host.lua, either streamer wands is corrupted or new/outdated")?
        .extract();

    let token = std::fs::read_to_string(token_path)
        .context("Failed to read streamer wands token, is the mod installed?")?;

    let (_, [token]) = lazy_regex!("return \"(.*?)\"")
        .captures(&token)
        .context("Malformed token.lua, either streamer wands is corrupted or new/outdated")?
        .extract();

    let ws_url = format!("{host}{token}");

    let msg_rx = poll_file(noita_dir.path.join("streamer-wands.json"))?;

    let mut retries = 0;
    loop {
        match send_loop(&ws_url, &msg_rx) {
            Err(e) => {
                if retries < 10 {
                    eprintln!("failed: {e}, retrying in 5 seconds");
                    retries += 1;
                    sleep(Duration::from_secs(5));
                } else {
                    eprintln!("failed 10 retries");
                    std::process::exit(1);
                }
            }
            Ok(reason) => {
                eprintln!("{reason}");
                std::process::exit(1);
            }
        }
    }
}
