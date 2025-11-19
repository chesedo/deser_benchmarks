fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/block.capnp")
        .run()
        .expect("compiling schema");
}
