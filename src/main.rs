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

/// Commands sent from the UI to the nodes to ask them to do stuff.
#[derive(Clone, Debug)]
enum UiCommand {
    ChangeColor(Color),
}

/// State shared between the UI and each node.
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

/// This function spawns a node listening on a certain port, connecting to
/// several other peer nodes in the peer_strings list. It also gets a ref
/// to the global state map that is shared among each node in this Proof of
/// concept. The states hold the channels used to communicate between the
/// UI thread and the logic thread.
async fn spawn_task(port: u16, peer_strings: &[&str], states: States) {
    // Create a comm channel between the UI thread and this task
    // so the UI can tell us what to do and vise versa
    let (tx, mut rx) = broadcast::channel(16);

    // Create the UI state
    let state = Arc::new(Mutex::new(UiState::new(tx)));

    {
        // Take an exclusive lock on the global state hashmap
        // and store our state in there, indexed by our current port
        let mut states = states.lock().unwrap();
        states.insert(port, state.clone());
    }

    // Create the node that we will be listening on. In this PoC
    let mut node = node::Node::<String>::new(port).await.start();

    // Connect to each of the TCP sockets in the peer_strings list
    for s in peer_strings {
        // Parse the address provided
        let addr: std::net::SocketAddr = s.parse().unwrap();
        // Keep trying to connect to the tcp socket using tokio. Loop until
        // it succeeds (TODO: make this just fail lol)
        let con = loop {
            println!("trying {} from {}", addr, port);
            if let Ok(stream) = tokio::net::TcpStream::connect(&addr).await {
                break stream;
            }
            println!("Failed!");
        };

        // Tell the node to connect to the peer we just joined to.
        node.send_cmd(node::MetaCommand::AddPeer(con, addr)).await;
    }

    /*
     * Now, the juicy stuff.
     *
     * read up on the select(2) system call linux provides to understand what select means here.
     * Basically it lets you wait on one or more futures and be notified of which one is ready when
     * it is. In this example, we wait on both data from the network and events from the UI thread.
     * Once one of these futures completes, either data_opt or cmd will be set, and the code inside
     * the block is run with the value sent over the channel.
     */
    loop {
        tokio::select! {

            /*
             * Wait for data over the network. In this example, the data is always going to be a
             * string, as we construct the nodes as node::Node::<String>::new(). This means
             * data_opt is a string. Once we have the data, we try to parse an expression from it
             * using regex (oooh scary) of the form "c(N,N,N,N)" where N is a floating point value
             * each representing R, G, B and A for the color of the node. This is an applciation
             * specific workload that we define for this applciation (duh, thats what that means)
             */
            data_opt = node.recv() => {
                if let Some(data) = data_opt {
                    let mut state = state.lock().unwrap();
                    /* Parse the regexj */
                    let re = Regex::new(r"c\((\d*\.?\d*),(\d*\.?\d*),(\d*\.?\d*),(\d*\.?\d*)\)").unwrap();
                    /* Grab the capture */
                    let captures = re.captures(&data.0);
                    // If the capture exists (regex matched), grab the colors from it, parsing
                    // them as floats. Then just log to the UI that the color was changed and set
                    // our color to the requested one.
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

            /*
             * In this branch of the select, we get events from the UI thread and handle them.
             * Here, the UI can tell us to ChangeColor to some other color. You can see the other
             * side of this code in the UI nonsense in main() below. All this does is ask the node
             * to broadcast messages over the network. Don't really worry about how all this works,
             * it's just a spooky method to get around rust's ownership model and whatnot. (The ui
             * thread cant broadcast itself, as this task owns the node mutably)
             *
             * In a real application (like when you use sensors, screens, lights, etc) you might
             * sit here and poll for events on a raspberry PI's GPIO pins, poking at the network
             * when something meaningful to your application happens. Maybe yall can work on that?
             * :^)
             *
             * TODO: maybe sit here and listen on a webserver? Look into the hyper library for a
             * (I think) good lib: https://github.com/hyperium/hyper
             */
            cmd = rx.recv() => {
                if let Ok(cmd) = cmd {
                    let mut state = state.lock().unwrap();
                    match cmd {
                        /* Given the color, format it to a string, and broadcast it. */
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
    /*
     * Run many nodes in the same process, as I didn't want to build a virtual machine
     * orchestration platform for an IPRO. Basically, this just defines the topology of
     * the network where each node connects to other nodes and they can broadcast
     * messages (strings in this case) through them as needed.
     */
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
    ];

    /* Sit and wait on all the futures above (this is just like joining on a bunch of pthreads in
     * C, except hipster and cool cause its rust */
    futures::future::join_all(futs).await;
}

fn main() {
    /* Create the states variable (a reference counted mutual lock of a hashmap) */
    let states: States = Default::default();
    {
        /* Spawn a thread for the nodes to run on. This is needed  */
        let states = states.clone();
        std::thread::spawn(move || comm_thread(states));
    }

    /* create the UI system */
    let system = support::init(file!());
    /* Sit in an infinite loop, getting events. This needs to live on the main thread, because
     * greybeard X11 developers don't know how to use mutex variables. Basically, it just makes a
     * window for each state in the globally shared states map defined above, and displays things
     * like each node's log along with some buttons and whatnot. Look into the C++ library imgui if
     * you are interested in this funky immediate mode UI stuff, but it's not really important for
     * the overall functioning of this application.
     *
     * Importantly, the buttons and color picker detect changes to their value and broadcast to the
     * state's tx channel so the node task in the other thread can be woken up in their select
     * loop.
     *
     * Again, it's important to note that this entire block of code would likely not be here in a
     * production app. This was just to have something to show dr. dan (only to have him say 'yeah
     * cool' and move on (still got an A tho))
     */
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
