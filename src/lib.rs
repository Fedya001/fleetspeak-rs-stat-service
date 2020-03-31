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
    use tempfile::NamedTempFile;
    pub use std::os::unix::fs::PermissionsExt;

    #[test]
    fn process_request_works_with_regular_file() -> Result<(), std::io::Error> {
        let temp_dir = tempfile::tempdir().unwrap();

        let secs_before_create = std::time::SystemTime::now().duration_since(
            std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs();

        let mut temp_file = NamedTempFile::new_in(&temp_dir)?;
        temp_file.write(b"Test tmp file content.")?;
        let path = temp_file.path().to_str().unwrap();

        std::fs::set_permissions(path, PermissionsExt::from_mode(0o644))?;
        std::fs::hard_link(path, temp_dir.path().join("hardlink1"))?;
        std::fs::hard_link(path, temp_dir.path().join("hardlink2"))?;

        let response = process_request(
            Request { path: String::from(path) });
        let secs_after_request = std::time::SystemTime::now().duration_since(
            std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs();

        assert_eq!(response.path, path);
        assert_eq!(response.size, 22);
        assert_eq!(response.mode & 0o777, 0o644);

        let inode = std::fs::metadata(
            temp_file.path().to_str().unwrap())?.ino();
        let extra = response.extra.unwrap();
        assert_eq!(extra.inode, inode);
        assert_eq!(extra.hardlinks_number, 3);

        let current_uid = users::get_current_uid();
        let owner = extra.owner.unwrap();
        assert_eq!(owner.uid, current_uid);
        assert_eq!(owner.name, get_name_by_uid(current_uid).unwrap());

        let current_gid = users::get_current_gid();
        let owner_group = extra.owner_group.unwrap();
        assert_eq!(owner_group.gid, current_uid);
        assert_eq!(owner_group.name, get_name_by_gid(current_gid).unwrap());

        let atime = extra.last_access_time.unwrap().seconds as u64;
        assert!(secs_before_create <= atime);
        assert!(secs_after_request >= atime);

        let mtime = extra.last_data_modification_time.unwrap().seconds as u64;
        assert!(secs_before_create <= mtime);
        assert!(secs_after_request >= mtime);

        let ctime = extra.last_status_change_time.unwrap().seconds as u64;
        assert!(secs_before_create >= ctime);
        assert!(secs_after_request <= ctime);

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
    fn response_status_fails_no_such_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nonexisting_file_path = temp_dir.path().join("does-not.exist");
        assert!(!eval_response_status(&std::fs::metadata(
            String::from(nonexisting_file_path.to_str().unwrap()))).success);
    }

    #[test]
    fn response_status_success_regular_file() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let status = eval_response_status(&std::fs::metadata(
            temp_file.path().to_str().unwrap()));
        assert!(status.success);
    }
}
