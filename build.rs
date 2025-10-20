fn main() {
    embed_resource::compile("assets/resources.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap()
}
