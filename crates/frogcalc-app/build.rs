fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let icon = std::path::Path::new(&manifest)
        .join("../../resources/icon.ico")
        .canonicalize()
        .expect("resources/icon.ico 不存在");
    println!("cargo:rerun-if-changed={}", icon.display());

    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("frogcalc.rc");
    let icon_path = icon.display().to_string().replace('\\', "/");
    std::fs::write(&out, format!("1 ICON \"{icon_path}\"\n")).expect("写入 rc 文件失败");

    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_required()
        .expect("嵌入 Windows 图标资源失败");
}
