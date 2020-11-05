#![deny(unused_must_use)]

use std::io::Write;

use itertools::Itertools;
use tokio::io::AsyncBufReadExt;
use imgui::*;


mod cli;
mod node;
mod peer;
mod proto;
mod support;


async fn spawn_task(port: u16, peer_strings: &[&str], shell: bool) {
    println!("port {}", port);

    let mut node = node::Node::new(port).await.start();
    for s in peer_strings {
        let addr: std::net::SocketAddr = s.parse().unwrap();

        let stream = loop {
            println!("trying {} from {}", addr, port);
            if let Ok(stream) = tokio::net::TcpStream::connect(&addr).await {
                break stream;
            }
            println!("Failed!");
        };


        node.send_cmd(node::MetaCommand::AddPeer(stream, addr)).await;
    }

    // run the interactive shell if we need to

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
        node.wait().await;
    }
}



/*
async fn comm_thread() {
    // tokio::spawn(comm_task(6999, &["127.0.0.1:7001", "127.0.0.1:7002"], true));
}
*/

#[tokio::main]
async fn main() {
    let futs = vec![
        spawn_task(7000, &["127.0.0.1:7001", "127.0.0.1:7003"], false),
        spawn_task(7001, &["127.0.0.1:7000", "127.0.0.1:7002", "127.0.0.1:7004"], false),
        spawn_task(7002, &["127.0.0.1:7001", "127.0.0.1:7005"], false),
        spawn_task(7003, &["127.0.0.1:7004", "127.0.0.1:7000"], false),
        spawn_task(7004, &["127.0.0.1:7003", "127.0.0.1:7005", "127.0.0.1:7001"], false),
        spawn_task(7005, &["127.0.0.1:7004", "127.0.0.1:7002"], false),
        spawn_task(6999, &["127.0.0.1:7001"], true),
    ];

    futures::future::join_all(futs).await;

    // std::thread::spawn(comm_thread);

    /*
    let system = support::init(file!());
    system.main_loop(move |_, ui| {
        Window::new(im_str!("Hello world"))
        .size([300.0, 100.0], Condition::FirstUseEver)
        .build(&ui, || {
            ui.text(im_str!("Hello world!"));
            ui.separator();
            let mouse_pos = ui.io().mouse_pos;
            ui.text(format!(
                "Mouse Position: ({:.1},{:.1})",
                mouse_pos[0], mouse_pos[1]
            ));
        });
    });
    */
}
