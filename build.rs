fn main() {
    embuild::espidf::sysenv::output();
    capnpc::CompilerCommand::new()
        .src_prefix("src/")
        .file("src/message.capnp")
        .run()
        .expect("compiling schema");
}
