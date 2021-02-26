fn main() {
    if cfg!(target_os = "linux") {
        // these pragmas are optional on my system but left as examples if you run into trouble.
        // println!("cargo:rustc-link-lib=X11");
        // println!("cargo:rustc-link-lib=Xcursor");
        // println!("cargo:rustc-link-lib=Xrandr");
        // println!("cargo:rustc-link-lib=Xi");
        println!("cargo:rustc-link-lib=vulkan");
    }
}
