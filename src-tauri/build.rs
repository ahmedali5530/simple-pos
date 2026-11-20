fn main() {
    tauri_build::build();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let launcher = std::path::Path::new("pgwrap.exe");
    let source = if launcher.is_file() {
        format!(
            "pub static BYTES: &[u8] = include_bytes!(r\"{}\");\n",
            launcher.canonicalize().unwrap().display()
        )
    } else {
        "pub static BYTES: &[u8] = &[];\n".to_string()
    };
    std::fs::write(out.join("pgwrap.rs"), source).unwrap();
    println!("cargo:rerun-if-changed=pgwrap.exe");
}
