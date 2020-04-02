mod lib;

fn main() {
    fleetspeak::startup("0.0.1")
        .expect("Failed to establish connection with Fleatspeak client");

    loop {
        let packet = fleetspeak::receive()
            .expect("Failed to receive a message from the Fleetspeak server");

        let request: lib::stat::Request = packet.data;
        let response = lib::stat::process_request(request);

        fleetspeak::send(fleetspeak::Packet {
            service: packet.service,
            kind: None,
            data: response,
        }).expect("Failed to send packet");
    }
}
