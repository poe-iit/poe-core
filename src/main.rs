#![deny(unused_must_use)]

mod cli;
mod node;
mod peer;
mod proto;
mod support;

use imgui::*;
use rand::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

type State = Arc<Mutex<UiState>>;
type States = Arc<Mutex<HashMap<u16, State>>>;

#[derive(Clone, Debug)]
enum UiCommand {
    Broadcast(String),
}

struct UiState {
    log: Vec<String>,
    tx: broadcast::Sender<UiCommand>,
}

impl UiState {
    pub fn new(tx: broadcast::Sender<UiCommand>) -> Self {
        Self {
            log: Default::default(),
            tx,
        }
    }
}

async fn spawn_task(port: u16, peer_strings: &[&str], states: States) {
    let (tx, mut rx) = broadcast::channel(16);

    let state = Arc::new(Mutex::new(UiState::new(tx)));

    {
        let mut states = states.lock().unwrap();
        states.insert(port, state.clone());
    }

    let mut node = node::Node::<String>::new(port).await.start();
    for s in peer_strings {
        let addr: std::net::SocketAddr = s.parse().unwrap();

        let stream = loop {
            println!("trying {} from {}", addr, port);
            if let Ok(stream) = tokio::net::TcpStream::connect(&addr).await {
                break stream;
            }
            println!("Failed!");
        };

        node.send_cmd(node::MetaCommand::AddPeer(stream, addr))
            .await;
    }

    loop {
        tokio::select! {
            /*
            _ = node.wait() => {
                println!("node dead!");
                break;
            },
            */

            data_opt = node.recv() => {
                if let Some(data) = data_opt {
                    let mut state = state.lock().unwrap();
                    state.log.push(format!("Got: '{:?}'", data));
                }
            }

            cmd = rx.recv() => {
                if let Ok(cmd) = cmd {
                    let mut state = state.lock().unwrap();
                    match cmd {
                        UiCommand::Broadcast(msg) => {
                            state.log.push(format!("Broadcasting: '{:?}'", msg));
                            drop(state);
                            node.broadcast(msg).await;
                        }
                    }
                }
            }
        };
    }

    // run the interactive shell if we need to

    /*
    if shell {
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        loop {
            print!(">>> ");
            std::io::stdout().flush().unwrap(); // ugh
            let mut buffer = String::new();
            if reader.read_line(&mut buffer).await.is_ok() {
                let line = buffer.trim();
                let mut args = line.split_ascii_whitespace();

                let arg0 = match args.next() {
                    Some(a) => a,
                    None => continue,
                };

                match arg0 {
                    "exit" => break,
                    "b" | "broadcast" => {
                        let msg = args.join(" ");
                        node.broadcast(msg).await;
                    }
                    _ => println!("Unknown command: '{}'", line),
                }
            } else {
                break;
            }
        }
        node.terminate().await;
    } else {
        // otherwise wait for the node to finish
    }
    */
}

#[tokio::main]
async fn comm_thread(states: States) {
    let futs = vec![
        spawn_task(7000, &["127.0.0.1:7001", "127.0.0.1:7003"], states.clone()),
        spawn_task(
            7001,
            &["127.0.0.1:7000", "127.0.0.1:7002", "127.0.0.1:7004"],
            states.clone(),
        ),
        spawn_task(7002, &["127.0.0.1:7001", "127.0.0.1:7005"], states.clone()),
        spawn_task(7003, &["127.0.0.1:7004", "127.0.0.1:7000"], states.clone()),
        spawn_task(
            7004,
            &["127.0.0.1:7003", "127.0.0.1:7005", "127.0.0.1:7001"],
            states.clone(),
        ),
        spawn_task(7005, &["127.0.0.1:7004", "127.0.0.1:7002"], states.clone()),
        // spawn_task(6999, &["127.0.0.1:7001"], true, states.clone()),
    ];

    futures::future::join_all(futs).await;
}

fn main() {
    let states: States = Default::default();
    {
        let states = states.clone();
        std::thread::spawn(move || comm_thread(states));
    }

    let system = support::init(file!());
    system.main_loop(move |_, ui| {
        let states = states.lock().unwrap();

        for (port, state) in states.iter() {
            let title = format!("Node {}", port);

            let mut rand = StdRng::seed_from_u64(*port as u64);

            Window::new(&imgui::ImString::new(title))
                .position(
                    [rand.gen::<f32>() * 1024f32, rand.gen::<f32>() * 768f32],
                    Condition::FirstUseEver,
                )
                .size([300.0, 400.0], Condition::FirstUseEver)
                .build(&ui, || {
                    ui.text(format!("Port: {}", port));
                    ui.separator();

                    ui.text_wrapped(im_str!("Here is a thing"));

                    if ui.small_button(im_str!("Broadcast")) {
                        state
                            .lock()
                            .unwrap()
                            .tx
                            .send(UiCommand::Broadcast(
                                format!("Fire at node {}", port).into(),
                            ))
                            .unwrap();
                    }

                    ui.separator();

                    ChildWindow::new("console")
                        .size([0.0, 0.0])
                        .border(true)
                        .build(ui, || {
                            for event in state.lock().unwrap().log.iter().rev() {
                                ui.text(format!("{}", event));
                            }
                        });

                    /*
                    let mut autoscroll = true;
                    ui.popup(im_str!("Options"), || {
                        ui.checkbox(im_str!("Auto-scroll"), &mut autoscroll);
                    });
                    if ui.small_button(im_str!("Options")) {
                        ui.open_popup(im_str!("Options"));
                    }
                    */
                });
        }
    });
}
