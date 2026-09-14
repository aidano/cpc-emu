fn main() {
    if std::env::var("CARGO_FEATURE_GUI").is_ok() {
        slint_build::compile("ui/cpc.slint").unwrap();
    }
}
