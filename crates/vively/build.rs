fn main() {
    println!("cargo:rerun-if-changed=windows.rc");
    println!("cargo:rerun-if-changed=../../assets/AppIcon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for("windows.rc", ["vively"], embed_resource::NONE)
            .manifest_required()
            .expect("could not embed the Windows application icon");
    }
}
