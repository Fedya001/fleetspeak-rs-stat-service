pub mod stat {
    include!(concat!(env!("OUT_DIR"), "/fleetspeak.stat.rs"));

    pub use std::os::unix::fs::MetadataExt;

    pub fn get_name_by_uid(uid: u32) -> Option<String> {
        users::get_user_by_uid(uid)?.name().to_str().map(String::from)
    }

    pub fn get_name_by_gid(gid: u32) -> Option<String> {
        users::get_group_by_gid(gid)?.name().to_str().map(String::from)
    }

    pub fn eval_response_status(metadata: &std::io::Result<std::fs::Metadata>)
                                -> response::Status {
        match metadata {
            Ok(_) => response::Status {
                success: true,
                error_details: String::new(),
            },
            Err(e) => response::Status {
                success: false,
                error_details: e.to_string(),
            }
        }
    }

    pub fn fill_stat_proto(meta: std::fs::Metadata) -> Response {
        Response {
            path: String::new(),
            size: meta.len() as i64,
            mode: meta.mode(),

            extra: Some(response::Extra {
                inode: meta.ino(),
                hardlinks_number: meta.nlink(),

                owner: Some(response::extra::User {
                    uid: meta.uid(),
                    name: get_name_by_uid(meta.uid()).unwrap_or_default(),
                }),
                owner_group: Some(response::extra::Group {
                    gid: meta.gid(),
                    name: get_name_by_gid(meta.gid()).unwrap_or_default(),
                }),

                last_access_time: Some(prost_types::Timestamp {
                    seconds: meta.atime(),
                    nanos: meta.atime_nsec() as i32,
                }),
                last_data_modification_time: Some(prost_types::Timestamp {
                    seconds: meta.mtime(),
                    nanos: meta.mtime_nsec() as i32,
                }),
                last_status_change_time: Some(prost_types::Timestamp {
                    seconds: meta.ctime(),
                    nanos: meta.ctime_nsec() as i32,
                }),
            }),
            status: None,
        }
    }

    pub fn process_request(request: Request) -> Response {
        let metadata = std::fs::metadata(&request.path);
        let status = eval_response_status(&metadata);

        let mut response = match metadata {
            Ok(meta) => fill_stat_proto(meta),
            Err(_) => Default::default()
        };

        response.path = request.path;
        response.status = Some(status);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::stat::*;
    use std::io::Write;

    #[test]
    fn process_request_works_with_regular_file() -> Result<(), std::io::Error> {
        let mut tmp_file = tempfile::NamedTempFile::new()?;
        tmp_file.write(b"Test tmp file content.")?;

        let path = tmp_file.path().to_str().unwrap();
        let response = process_request(
            Request { path: String::from(path) });
        let meta = std::fs::metadata(path)?;

        assert_eq!(response.path, path);
        assert_eq!(response.size, meta.len() as i64);
        assert_eq!(response.mode, meta.mode());

        let extra = response.extra.unwrap();
        assert_eq!(extra.inode, meta.ino());
        assert_eq!(extra.hardlinks_number, meta.nlink());

        let owner = extra.owner.unwrap();
        assert_eq!(owner.uid, meta.uid());
        assert_eq!(owner.name,
                   get_name_by_uid(meta.uid()).unwrap());

        let owner_group = extra.owner_group.unwrap();
        assert_eq!(owner_group.gid, meta.gid());
        assert_eq!(owner_group.name,
                   get_name_by_gid(meta.gid()).unwrap());

        let atime = extra.last_access_time.unwrap();
        assert_eq!(atime.seconds, meta.atime());
        assert_eq!(atime.nanos, meta.atime_nsec() as i32);

        let mtime = extra.last_data_modification_time.unwrap();
        assert_eq!(mtime.seconds, meta.mtime());
        assert_eq!(mtime.nanos, meta.mtime_nsec() as i32);

        let ctime = extra.last_status_change_time.unwrap();
        assert_eq!(ctime.seconds, meta.ctime());
        assert_eq!(ctime.nanos, meta.ctime_nsec() as i32);

        assert!(response.status.unwrap().success);
        Ok(())
    }

    #[test]
    fn process_request_works_with_nonexisting_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nonexisting_file_path = temp_dir.path().join("does-not.exist");

        let response = process_request(Request {
            path: String::from(nonexisting_file_path.to_str().unwrap())
        });
        assert!(!response.status.unwrap().success);
    }

    #[test]
    fn response_status_no_false_positive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nonexisting_file_path = temp_dir.path().join("does-not.exist");
        assert!(!eval_response_status(&std::fs::metadata(
            String::from(nonexisting_file_path.to_str().unwrap()))).success);
    }

    #[test]
    fn response_status_no_false_negative() {
        let tmp_file = tempfile::NamedTempFile::new().unwrap();
        let status = eval_response_status(&std::fs::metadata(
            tmp_file.path().to_str().unwrap()));
        assert!(status.success);
    }
}
