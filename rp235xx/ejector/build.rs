//! Set up linker scripts for the rp235x-hal examples

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put the linker script somewhere the linker can find it
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search={}", out.display());

    // The file `memory.x` is loaded by cortex-m-rt's `link.x` script, which
    // is what we specify in `.cargo/config.toml` for Arm builds
    #[cfg(feature = "rp2350")]
    {
        let memory_x = include_bytes!("memory-rp2350.x");
        let mut f = File::create(out.join("memory.x")).unwrap();
        f.write_all(memory_x).unwrap();
        println!("cargo:rustc-link-search={}", out.display());
    }

    println!("cargo:rerun-if-changed=memory-rp2350.x");

    println!("cargo:rerun-if-changed=build.rs");
}
