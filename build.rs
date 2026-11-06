fn main() {
    #[cfg(target_os = "windows")]
    {
        let ico_path = "assets/icon.ico";
        if std::path::Path::new(ico_path).exists() {
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let rc_path = format!("{}/app.rc", out_dir);
            let res_path = format!("{}/app.res", out_dir);
            let canonical = std::fs::canonicalize(ico_path)
                .unwrap()
                .display()
                .to_string()
                .replace('\\', "/");
            let _ = std::fs::write(&rc_path, format!("1 ICON \"{}\"\n", canonical));

            // Try windres (MinGW/Git toolchain)
            let windres_status = std::process::Command::new("windres")
                .args(["-i", &rc_path, "-o", &res_path, "-O", "coff"])
                .status();

            if let Ok(s) = windres_status {
                if s.success() {
                    println!("cargo:rustc-link-arg={}", res_path);
                }
            }
        }
    }
}
