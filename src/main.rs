extern crate clap;
extern crate hexplay;
extern crate pnet;

use clap::{value_t, App, Arg};

use pnet::datalink::Channel::Ethernet;
use pnet::datalink::{self, Config, NetworkInterface};

use std::io;
use std::time::{Duration, Instant};

fn print_interfaces() {
    println!("Detected Network Interfaces:");
    let list_of_interfaces = datalink::interfaces();
    for interface in list_of_interfaces {
        println!("{}", interface.name);
        for ipaddr in interface.ips {
            println!("  IP: {}", ipaddr);
        }
    }
}

// Invoke as echo <interface name>
fn main() {
    let matches = App::new("NetHex")
        .version("0.1.0")
        .author("Jack Newman")
        .about("A small utility for reading / writing directly to a network interface")
        .arg(
            Arg::with_name("timeout")
                .short("t")
                .long("timeout")
                .takes_value(true)
                .help("Time before exiting the program"),
        )
        .arg(
            Arg::with_name("count")
                .short("c")
                .long("count")
                .takes_value(true)
                .help("Number of packet to receive before exiting")
                .default_value("-1"),
        )
        .arg(
            Arg::with_name("list")
                .short("l")
                .long("list")
                .help("List network interfaces"),
        )
        .arg(
            Arg::with_name("interface")
                .help("The network interface to send/read from")
                .required_unless("list"),
        )
        .arg(
            Arg::with_name("bytes")
                .help("A hex string of raw bytes to send to the interface e.g. 11EE22FF"),
        )
        .get_matches();

    // println!("{:?}", matches);
    if matches.is_present("list") {
        print_interfaces();
        std::process::exit(0);
    };

    let rx_timeout = value_t!(matches, "timeout", u64)
        .ok()
        .map(|time| Duration::from_secs(time));
    let mut rx_countlimit = value_t!(matches, "count", i64).unwrap();

    // Grab the input interface. No error checking as clap will exit if it does not exist
    let interface_name = matches.value_of("interface").unwrap();

    // Find the network interface with the provided name
    let interface_names_match = |iface: &NetworkInterface| iface.name == interface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces
        .into_iter()
        .filter(interface_names_match)
        .next()
        .expect("Could not find the network interface");

    // Create a new channel, dealing with layer 2 packets
    let mut datalink_config = Config::default();
    // Set the timeout of the socket read to 10ms
    datalink_config.read_timeout = Some(std::time::Duration::new(0, 1e7 as u32)); 

    let (mut tx, mut rx) = match datalink::channel(&interface, datalink_config) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("Error while creating datalink channel: {:?}", e),
    };

    // Decode the hex input if the user specified one
    if let Some(arg) = matches.value_of("bytes") {
        extern crate hex;
        use hex::FromHex;
        let bytes = match Vec::from_hex(arg) {
            Ok(bytes) => bytes,
            Err(e) => {
                println!("{}", e);
                std::process::exit(1);
            }
        };
        // Transmit those bytes
        println!("Sending bytes: {:X?}", bytes);
        let res =  tx.send_to(&bytes, None).unwrap();
        if let Err(error) = res {
                println!("{:?}", error);
                std::process::exit(1);
            };
    }

    // Now do the Rx part
    let now = Instant::now();
    while rx_countlimit != 0 {
        match rx.next() {
            Ok(packet) => {
                println!("----- Recv Packet -----");
                use hexplay::HexViewBuilder;
                let view = HexViewBuilder::new(packet).row_width(16).finish();
                println!("{}", view);
                rx_countlimit -= 1;
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                // Timeout errors are fine. Ignore.
            }
            Err(e) => {
                // If any other error occurs, we can handle it here
                panic!("An error occurred while reading: {:?}", e);
            }
        }
        if let Some(rx_timeout) = rx_timeout {
            // If there is a timeout enabled. Check it
            if now.elapsed() > rx_timeout {
                std::process::exit(0);
            }
        }
    }
}
