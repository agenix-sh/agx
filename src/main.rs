fn main() {
    if let Err(error) = agx::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
