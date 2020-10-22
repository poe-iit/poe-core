use std::io::Write;

use tokio::io::AsyncBufReadExt;

mod cli;
mod node;
mod peer;
mod proto;

use node::Node;

#[tokio::main]
async fn main() {
    let matches = cli::build_cli().get_matches();

    let port = matches
        .value_of("port")
        .unwrap()
        .parse::<u16>()
        .expect("Port is not an integer");

    let mut node: Node<String> = Node::new(port).await;

    let peer_strings = matches.value_of("peers").unwrap().split(',');

    for s in peer_strings {
        let addr = s.parse().unwrap();
        node.known_peers.insert(addr);
    }

    let mut node = node.start();

    // run the interactive shell if we need to

    if matches.is_present("shell") {
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        loop {
            print!(">>> ");
            std::io::stdout().flush().unwrap(); // ugh
            let mut buffer = String::new();
            if reader.read_line(&mut buffer).await.is_ok() {
                let line = buffer.trim();
                let parts = line.split_ascii_whitespace().collect::<Vec<_>>();
                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "exit" => break,
                    "b" => {
                        let msg = parts[1..].join(" ");
                        node.broadcast(msg).await;
                    }
                    _ => println!("Unknown '{}'", line),
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
