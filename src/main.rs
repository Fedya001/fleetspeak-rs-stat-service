use fleetspeak::Packet;
use std::ffi::CString;
use users::{get_user_by_uid, get_group_by_gid};

pub mod stat {
    include!(concat!(env!("OUT_DIR"), "/fleetspeak.stat.rs"));
}

fn libc_stat_syscall(path: &str) -> libc::stat {
    unsafe {
        let path = CString::new(path).unwrap();
        let mut statbuf: libc::stat = std::mem::zeroed();

        if libc::stat(path.as_ptr(), &mut statbuf) == 0 {
            statbuf
        } else {
            // TODO : handle error
            std::mem::zeroed()
        }
    }
}

fn get_name_by_uid(uid: u32) -> Option<String> {
    match get_user_by_uid(uid) {
        Some(user) => {
            match user.name().to_str() {
                Some(username) => Some(String::from(username)),
                None => None,
            }
        }
        None => None,
    }
}

fn get_name_by_gid(gid: u32) -> Option<String> {
    match get_group_by_gid(gid) {
        Some(group) => {
            match group.name().to_str() {
                Some(group_name) => Some(String::from(group_name)),
                None => None,
            }
        }
        None => None
    }
}

fn process_request(request: stat::Request) -> stat::Response {
    let statbuf = libc_stat_syscall(&request.path[..]);

    stat::Response {
        path: request.path,
        size: statbuf.st_size,
        mode: statbuf.st_mode,

        extra: Some(stat::response::Extra {
            inode: statbuf.st_ino,
            hardlinks_number: statbuf.st_nlink,

            owner: Some(stat::response::extra::User {
                uid: statbuf.st_uid,
                name: get_name_by_uid(statbuf.st_uid).unwrap_or_default(),
            }),
            owner_group: Some(stat::response::extra::Group {
                gid: statbuf.st_gid,
                name: get_name_by_gid(statbuf.st_gid).unwrap_or_default(),
            }),

            last_access_time: Some(prost_types::Timestamp {
                seconds: statbuf.st_atime,
                nanos: statbuf.st_atime_nsec as i32,
            }),
            last_data_modification_time: Some(prost_types::Timestamp {
                seconds: statbuf.st_mtime,
                nanos: statbuf.st_mtime_nsec as i32,
            }),
            last_status_change_time: Some(prost_types::Timestamp {
                seconds: statbuf.st_ctime,
                nanos: statbuf.st_ctime_nsec as i32,
            }),
        }),
    }
}

fn main() {
    fleetspeak::startup("0.0.1")
        .expect("Failed to establish connection with Fleatspeak client");

    loop {
        let packet = fleetspeak::receive()
            .expect("Failed to receive a message from the Fleetspeak server");

        let request: stat::Request = packet.data;
        let response = process_request(request);

        fleetspeak::send(Packet {
            service: packet.service,
            kind: None,
            data: response,
        }).expect("Failed to send packet");
    }
}
