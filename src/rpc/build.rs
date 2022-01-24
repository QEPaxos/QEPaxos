use std::error::Error;
use std::process::exit;

fn run() -> Result<(), Box<dyn Error>> {
    let proto_files = &[
        "proto/raft_rpc.proto",
        "proto/qepaxos_rpc.proto",
        "proto/epaxos.proto",
    ];

    tonic_build::configure()
        .out_dir("src")
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile(proto_files, &["proto"])?;
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        exit(1);
    }

    // Tells cargo to only rebuild if the proto file changed
    println!("cargo:rerun-if-changed=proto");
}
