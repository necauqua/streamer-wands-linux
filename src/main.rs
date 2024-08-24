use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, RecvTimeoutError},
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use include_dir::include_dir;
use lazy_regex::lazy_regex;
use notify::{
    event::{AccessKind, AccessMode},
    Event, EventKind, INotifyWatcher, RecursiveMode, Watcher,
};
use steamlocate::SteamDir;
use tungstenite::{Message, WebSocket};

fn snoop_ws_url(
    noita_dir: &Path,
    host_override: Option<String>,
    token_override: Option<String>,
) -> Result<String> {
    let host = match host_override {
        Some(host) => host,
        _ => {
            let host_path = noita_dir.join("mods/streamer_wands/files/ws/host.lua");

            let host = std::fs::read_to_string(host_path)
                .context("Failed to read streamer wands host, is the mod installed?")?;

            let (_, [host]) = lazy_regex!("HOST_URL = \"(.*?)\"")
                .captures(&host)
                .context("Malformed host.lua, either streamer wands is corrupted or new/outdated")?
                .extract();

            if host.ends_with('/') {
                host.to_owned()
            } else {
                format!("{host}/")
            }
        }
    };

    let token = match token_override {
        Some(token) => token,
        _ => {
            let token_path = noita_dir.join("mods/streamer_wands/token.lua");

            let token = std::fs::read_to_string(token_path)
                .context("Failed to read streamer wands token, is the mod installed?")?;

            let (_, [token]) = lazy_regex!("return \"(.*?)\"")
                .captures(&token)
                .context("Malformed token.lua, either streamer wands is corrupted or new/outdated")?
                .extract();

            token.to_owned()
        }
    };

    Ok(format!("{host}{token}"))
}

fn install_patch_mod(noita_dir: &Path) -> Result<()> {
    let mod_dir = noita_dir.join("mods/streamer_wands_linux");

    std::fs::create_dir_all(&mod_dir)?;

    include_dir!("patch-mod")
        .extract(mod_dir)
        .context("Failed to install the streamer wands patch mod")?;

    Ok(())
}

fn poll_file(file: &Path) -> Result<(Receiver<String>, INotifyWatcher)> {
    let (messages_tx, messages_rx) = std::sync::mpsc::channel();

    let mut watcher = notify::recommended_watcher({
        let file = file.to_owned();
        move |e: notify::Result<Event>| {
            let Ok(e) = e else {
                return;
            };
            let EventKind::Access(AccessKind::Close(AccessMode::Write)) = e.kind else {
                return;
            };
            if let Ok(data) = std::fs::read_to_string(&file) {
                messages_tx.send(data.clone()).unwrap();
            }
        }
    })?;
    watcher.watch(file, RecursiveMode::NonRecursive)?;

    Ok((messages_rx, watcher))
}

struct Pinger {
    last_ping: Instant,
    ping_interval: Duration,
    ping_message: Message,
}

impl Pinger {
    fn new(ping_interval: Duration, ping_message: Message) -> Self {
        Self {
            last_ping: Instant::now(),
            ping_interval,
            ping_message,
        }
    }

    fn ping<S: Read + Write>(&mut self, socket: &mut WebSocket<S>) -> Result<()> {
        socket.send(self.ping_message.clone())?;
        self.last_ping = Instant::now();
        Ok(())
    }

    fn maybe_ping<S: Read + Write>(&mut self, socket: &mut WebSocket<S>) -> Result<bool> {
        if self.last_ping.elapsed() > self.ping_interval {
            self.ping(socket)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

struct Counter {
    prefix: String,
    count: u64,
}

impl Counter {
    fn new(prefix: impl ToString) -> Self {
        Self {
            prefix: prefix.to_string(),
            count: 0,
        }
    }

    fn redraw(&self) {
        eprintln!("\x1b[F{}{}", self.prefix, self.count);
    }

    fn tick(&mut self) {
        self.count += 1;
        self.redraw();
    }
}

fn send_loop(ws_url: &str, msg_rx: &Receiver<String>, retries: &mut u32) -> Result<&'static str> {
    let (mut socket, response) = tungstenite::connect(ws_url)?;

    let s = response.status();
    if !s.is_success() && !s.is_informational() {
        if s.is_client_error() {
            return Ok("{s} response from the server, bad token? Try to re-auth");
        }
        bail!("{s} response from the server, is it down?.");
    }

    let mut counter = Counter::new("sent messages: ");
    eprintln!();
    counter.redraw();

    let mut pinger = Pinger::new(Duration::from_secs(5), Message::Text("im alive".into()));

    loop {
        match msg_rx.recv_timeout(pinger.ping_interval) {
            Ok(msg) => {
                pinger.maybe_ping(&mut socket)?;
                socket.send(Message::Text(msg))?;
                counter.tick();
                *retries = 0;
            }
            Err(RecvTimeoutError::Timeout) => pinger.ping(&mut socket)?,
            Err(RecvTimeoutError::Disconnected) => {
                break Ok("inotify channel disconnected");
            }
        }
    }
}

/// A hacky workaround for the streamer wands mod not being able to connect to
/// the onlywands websocket server to send data on Linux, due to the
/// pollnet.dll library not working under Proton.
///
/// It installs a tiny Noita mod that patches the streamer wands mod to write
/// the data to a file, and then looks for changes in that file and sends them
/// to the onlywands websocket outside of the win32 game running through wine.
#[derive(clap::Parser)]
struct Args {
    /// Do not connect to the onlywands websocket and print the messages to
    /// stdout instead
    #[arg(short, long)]
    dry_run: bool,
    /// Do not install the patch mod in the noita mods folder
    #[arg(short = 'D', long)]
    dont_patch: bool,
    /// Override the websocket host. By default it's scraped from the streamer
    /// wands installation (so either `wss://onlywands.com` or
    /// `wss://dev.onlywands.com`)
    #[arg(short = 'H', long)]
    host: Option<String>,
    /// Override the JWT used to authenticate to the websocket. By default it's
    /// scraped from the streamer wands installation
    #[arg(short = 'T', long)]
    token: Option<String>,
    /// Override the noita installation dir - usually the steam folder is
    /// automatically discovered.
    #[arg()]
    noita_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // TLS without system dependency on openssl
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let noita_dir = match args.noita_dir {
        Some(dir) => dir,
        _ => {
            let mut steam = SteamDir::locate().context("Steam not found")?;
            let noita_dir = steam.app(&881100).context("Noita not found")?;
            noita_dir.path.clone()
        }
    };

    let ws_url = snoop_ws_url(&noita_dir, args.host, args.token)?;

    if !args.dont_patch {
        // install after snooping cuz now we're sure something
        // looking an awful lot like streamer wands is installed
        install_patch_mod(&noita_dir)?;
    }

    let (msg_rx, _watcher) = poll_file(&noita_dir.join("streamer-wands.json"))?;

    if args.dry_run {
        loop {
            println!("{}", msg_rx.recv()?);
        }
    }

    let mut retries = 0;
    loop {
        match send_loop(&ws_url, &msg_rx, &mut retries) {
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
