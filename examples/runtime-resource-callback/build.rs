

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::AsAbsolutePath);

    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}