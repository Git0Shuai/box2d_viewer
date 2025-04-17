fn main() {
    //
    // 1. make sure cxxlibbridge1 is available during linking the c++ main
    //    https://github.com/dtolnay/cxx/issues/875#issuecomment-913104697
    //
    // 2. extra link library: ws2_32.lib ntdll.lib userenv.lib)
    //

    let _build = cxx_build::bridge("src/lib.rs");

    println!("cargo::rerun-if-changed=src/lib.rs");
    println!("cargo::rerun-if-changed=src/main.cpp");
}
