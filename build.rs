fn main() {
    prost_build::compile_protos(&["src/proto/stat.proto"],
                                &["src/proto"]).unwrap();
}
