use tokio::net::UdpSocket;
use tokio::prelude::*;
extern crate clap;
use clap::{Arg, App, SubCommand};


#[tokio::main]
async fn main()  {

     let matches = App::new("Power over Ethernet Node")
                          .version("1.0")
                          .author("Just a bunch of dumbies")
                          .about("Mesh communication and packet bouncing")
                          .arg(Arg::with_name("port")
                               .short("p")
                               .long("port")
                               .value_name("PORT")
                               .help("Sets a port to listen from")
                               .required(true)
                               .takes_value(true))
                          .get_matches();


    let port: i16 = matches.value_of("port").unwrap().parse::<i16>().expect("Port is not an integer");


    let addr = format!("127.0.0.1:{}", port);
    println!("Listening at {}", addr);

    let mut socket = UdpSocket::bind(addr).await.unwrap();
    let mut buf = [0u8; 0xFFFF]; // maximum udp dgram size

    loop {
        let (len, peer) = socket.recv_from(&mut buf).await.unwrap();
        dbg!(len, &buf[0..len], peer);

    }
}
