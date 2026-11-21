//! Link PAM without pam-devel (Fedora ships libpam.so.0 without a .so symlink).

fn main() {
    // Prefer the unversioned name when pam-devel is installed.
    let lib = std::path::Path::new("/usr/lib64/libpam.so");
    let lib32 = std::path::Path::new("/usr/lib/libpam.so");
    if lib.exists() || lib32.exists() {
        println!("cargo:rustc-link-lib=pam");
    } else {
        // Runtime library only — tell ld to use the SONAME file directly.
        println!("cargo:rustc-link-arg=-l:libpam.so.0");
    }
}
