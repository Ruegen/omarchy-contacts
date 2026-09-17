use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::thread;

use omarchy_contactsd::daemon::{serve_socket_shared, serve_stdio_shared};
use omarchy_contactsd::paths::Layout;
use omarchy_contactsd::Daemon;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprint_help();
        std::process::exit(0);
    }
    let code = match run(&args) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{}", e);
            1
        }
    };
    std::process::exit(code);
}

fn run(args: &[String]) -> io::Result<()> {
    if args.first().map(|s| s.as_str()) == Some("ping") {
        return ping();
    }
    let daemon = Daemon::new()?;
    let layout = Layout::open()?;
    let shared = Arc::new(Mutex::new(daemon));
    if args.iter().any(|a| a == "--socket") {
        return serve_socket_shared(&layout.socket_path(), shared);
    }
    let sock = layout.socket_path();
    let bg = Arc::clone(&shared);
    thread::spawn(move || {
        let _ = serve_socket_shared(&sock, bg);
    });
    serve_stdio_shared(shared)
}

fn ping() -> io::Result<()> {
    let layout = Layout::open()?;
    let mut s = UnixStream::connect(layout.socket_path())?;
    s.write_all(b"{\"id\":1,\"cmd\":\"ping\"}\n")?;
    s.flush()?;
    let mut buf = String::new();
    std::io::BufRead::read_line(&mut std::io::BufReader::new(s), &mut buf)?;
    print!("{buf}");
    Ok(())
}

fn eprint_help() {
    let _ = writeln!(
        io::stderr(),
        "omarchy-contactsd — local address book for Omarchy\n\n  (no args)   JSON lines on stdin/stdout, plus a Unix socket\n  --socket    socket only\n  ping        ask a running helper if it is alive\n"
    );
}
