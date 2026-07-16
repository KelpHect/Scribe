fn main() {
    println!("cargo:rerun-if-changed=../../assets/scribe-icon-v2.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("../../assets/scribe-icon-v2.ico")
            .set("ProductName", "Scribe")
            .set("FileDescription", "Scribe ESO Addon Manager")
            .set("OriginalFilename", "Scribe-windows-amd64.exe")
            .set("InternalName", "Scribe")
            .compile()
            .expect("compile Scribe Windows resources");
    }
}
