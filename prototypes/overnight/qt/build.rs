use cxx_qt_build::{CxxQtBuilder, QmlModule};
fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("Overnight.Reader").qml_files(["qml/Main.qml", "qml/Smoke.qml"]),
    )
    .crate_include_root(Some("native".into()))
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("DBus")
    .files(["src/bridge.rs", "src/native.rs"])
    .cpp_files([
        "native/launch.cpp",
        "native/theme.cpp",
        "native/theme.h",
        "native/support.h",
        "native/support.cpp",
    ])
    .build();
    println!("cargo:rerun-if-changed=native/launch.cpp");
    println!("cargo:rerun-if-changed=native/launch.h");
}
