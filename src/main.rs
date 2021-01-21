#![deny(unused_must_use)]

mod node;
mod peer;
mod proto;
mod support;

use imgui::*;
use rand::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

type State = Arc<Mutex<UiState>>;
type States = Arc<Mutex<HashMap<u16, State>>>;

type Color = [f32; 4];

#[derive(Clone, Debug)]
enum UiCommand {
    Broadcast(String),
    ChangeColor(Color),
}

struct UiState {
    log: Vec<String>,
    color: Color,
    tx: broadcast::Sender<UiCommand>,
}

impl UiState {
    pub fn new(tx: broadcast::Sender<UiCommand>) -> Self {
        Self {
            log: Default::default(),
            color: Default::default(),
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

            data_opt = node.recv() => {
                if let Some(data) = data_opt {
                    let mut state = state.lock().unwrap();
                    let re = Regex::new(r"c\((\d*\.?\d*),(\d*\.?\d*),(\d*\.?\d*),(\d*\.?\d*)\)").unwrap();
                    let captures = re.captures(&data.0);
                    if let Some(captures) = captures {
                        let mut newcolor = state.color;
                        newcolor[0] = captures[1].parse().unwrap();
                        newcolor[1] = captures[2].parse().unwrap();
                        newcolor[2] = captures[3].parse().unwrap();
                        newcolor[3] = captures[4].parse().unwrap();
                        state.log.push(format!("told to change color to: '{:?}'", newcolor));
                        state.color = newcolor;
                    }
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
                        UiCommand::ChangeColor(newcolor) => {
                            state.log.push(format!("Changing color to: '{:?}'", newcolor));
                            node.broadcast(
                                format!("c({},{},{},{})", newcolor[0], newcolor[1], newcolor[2], newcolor[3])
                                    .into()).await;
                            state.color = newcolor;
                        }
                    }
                }
            }
        };
    }
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
                .size([300.0, 240.0], Condition::FirstUseEver)
                .build(&ui, || {
                    ui.text(format!("Port: {}", port));
                    ui.separator();

                    let mut color = state.lock().unwrap().color;
                    let ce = ColorEdit::new(im_str!("color_edit"), &mut color);
                    if ce.build(&ui) {
                        state
                            .lock()
                            .unwrap()
                            .tx
                            .send(UiCommand::ChangeColor(color))
                            /*
                             */
                            .unwrap();
                    }

                    ChildWindow::new("console")
                        .size([0.0, 0.0])
                        .border(true)
                        .build(ui, || {
                            for event in state.lock().unwrap().log.iter().rev() {
                                ui.text(format!("{}", event));
                            }
                        });
                });
        }
    });
}
