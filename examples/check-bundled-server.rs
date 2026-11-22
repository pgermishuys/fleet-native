//! `cargo run --example check-bundled-server -- <server-dir> <package-dir>`: proves a package carries the staged server
//! as built (row 25.2). Exits 1 when a file is missing, extra or different.
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [server, package] = args.as_slice() else {
        eprintln!("usage: check-bundled-server <server-dir> <package-dir>");
        std::process::exit(2);
    };
    let check = fleet_native::desktop::bundle::check_bundled_server(Path::new(server), Path::new(package));
    for line in &check.lines {
        if line.starts_with("::error::") {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    }
    std::process::exit(if check.ok { 0 } else { 1 });
}
