use fleetspeak::Packet;
use std::ffi::CString;
use users::{get_user_by_uid, get_group_by_gid};
use std::io::Error;

pub mod stat {
    include!(concat!(env!("OUT_DIR"), "/fleetspeak.stat.rs"));
}

fn libc_stat_syscall(path: &str) -> Option<libc::stat> {
    unsafe {
        let path = CString::new(path).unwrap();
        let mut statbuf: libc::stat = std::mem::zeroed();

        if libc::stat(path.as_ptr(), &mut statbuf) == 0 {
            Some(statbuf)
        } else {
            None
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

fn eval_response_status(statbuf: Option<libc::stat>) -> stat::response::Status {
    match statbuf {
        Some(_) => stat::response::Status {
            success: true,
            error_details: String::new(),
        },
        None => stat::response::Status {
            success: false,
            error_details: Error::last_os_error().to_string(),
        }
    }
}

fn process_request(request: stat::Request) -> stat::Response {
    let statbuf = libc_stat_syscall(&request.path[..]);
    let status = eval_response_status(statbuf);

    let statbuf = unsafe {
        statbuf.unwrap_or(std::mem::zeroed())
    };

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
        status: Some(status),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::fs::metadata;
    use std::io::Write;
    use std::os::linux::fs::MetadataExt;
    use users::{all_users, group_access_list};

    #[test]
    fn stat_syscall_works_with_regular_file() -> Result<(), Error> {
        let mut tmp_file = NamedTempFile::new()?;
        tmp_file.write(b"Test tmp file content.")?;

        let path = tmp_file.path().to_str().unwrap();
        let statbuf = libc_stat_syscall(path).unwrap();
        let meta = metadata(path)?;

        assert_eq!(statbuf.st_size, meta.len() as i64);
        assert_eq!(statbuf.st_mode, meta.st_mode());
        assert_eq!(statbuf.st_ino, meta.st_ino());
        assert_eq!(statbuf.st_nlink, meta.st_nlink());

        assert_eq!(statbuf.st_uid, meta.st_uid());
        assert_eq!(statbuf.st_gid, meta.st_gid());

        assert_eq!(statbuf.st_atime, meta.st_atime());
        assert_eq!(statbuf.st_atime_nsec, meta.st_atime_nsec());
        assert_eq!(statbuf.st_mtime, meta.st_mtime());
        assert_eq!(statbuf.st_mtime_nsec, meta.st_mtime_nsec());
        assert_eq!(statbuf.st_ctime, meta.st_ctime());
        assert_eq!(statbuf.st_ctime_nsec, meta.st_ctime_nsec());

        Ok(())
    }

    #[test]
    fn stat_syscall_works_with_nonexisting_file() {
        let statbuf = libc_stat_syscall(
            "this/file/does/not-exist.i.believe");
        assert!(statbuf.is_none());
    }

    #[test]
    fn username_matches_uid() {
        let iter = unsafe { all_users() };
        for user in iter {
            assert_eq!(get_name_by_uid(user.uid()).unwrap(),
                       String::from(user.name().to_str().unwrap()));
        }

        // Test on non-existing user
        let uid = u32::max_value() - 42;
        assert!(get_name_by_uid(uid).is_none());
    }

    #[test]
    fn group_name_matches_gid() {
        for group in group_access_list().expect("Error looking up groups") {
            assert_eq!(get_name_by_gid(group.gid()).unwrap(),
                       String::from(group.name().to_str().unwrap()));
        }

        // Test on non-existing group
        let gid = u32::max_value() - 42;
        assert!(get_name_by_gid(gid).is_none());
    }

    #[test]
    fn response_status_correct() {
        assert!(!eval_response_status(None).success);

        let statbuf = unsafe { std::mem::zeroed() };
        assert!(eval_response_status(Some(statbuf)).success);
    }
}
