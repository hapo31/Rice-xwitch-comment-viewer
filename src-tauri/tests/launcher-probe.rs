//! Isolated Windows launch fixture, compiled from reviewed source in CI.
//! No network, shell, elevation or application settings access.
use std::io::Write;

fn main() {
    let output = std::env::var_os("RICE_LAUNCH_PROBE_OUT").expect("fixture output path");
    let mut record = Vec::new();
    writeln!(record, "cwd={}", std::env::current_dir().unwrap().display()).unwrap();
    for argument in std::env::args().skip(1) {
        writeln!(record, "arg={argument}").unwrap();
    }
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .unwrap();
    file.write_all(&record).unwrap();
    file.sync_all().unwrap();
}
