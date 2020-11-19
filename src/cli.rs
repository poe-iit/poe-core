/*
use clap::{App, Arg};

pub fn build_cli() -> App<'static, 'static> {
    App::new("Power over Ethernet Node")
        .version("1.0")
        .author("Just a bunch of dumbies")
        .about("Mesh communication and packet bouncing")
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Sets a port to listen from")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("peers")
                .short("c")
                .long("peers")
                .help("A CSV list of topologically adjacent servers")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("shell")
                .short("i")
                .long("shell")
                .help("run the interactive shell")
                .required(false)
                .takes_value(false),
        )
}
*/
