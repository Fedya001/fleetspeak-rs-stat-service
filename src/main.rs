pub mod stat {
    include!(concat!(env!("OUT_DIR"), "/fleetspeak.stat.rs"));
}

fn main() {
    fleetspeak::startup("0.0.1")
        .expect("Failed to establish connection with Fleatspeak client");
}
