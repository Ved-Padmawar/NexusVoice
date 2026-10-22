//! One running instance. A second launch hands its arguments to the first over
//! a local socket and exits: a dictation flag (the Wayland path, where the
//! desktop owns the shortcut) is acted on, anything else reopens the window.

use std::io::{BufRead, BufReader, Write};

use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Name, Stream};

use crate::events::{self, Event};

fn socket_name() -> std::io::Result<Name<'static>> {
    format!("{}.sock", super::paths::IDENTIFIER).to_ns_name::<GenericNamespaced>()
}

/// Claim the instance lock, or forward `args` to the instance holding it.
/// Returns `false` when another instance is running and this one should exit.
pub fn acquire(args: &[String]) -> bool {
    let Ok(name) = socket_name() else {
        return true;
    };
    if let Ok(mut stream) = Stream::connect(name.clone()) {
        let _ = stream.write_all(args.join("\n").as_bytes());
        return false;
    }
    match ListenerOptions::new().name(name).create_sync() {
        Ok(listener) => std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let args: Vec<String> = BufReader::new(stream)
                        .lines()
                        .map_while(Result::ok)
                        .collect();
                    if !crate::remote::handle_args(&args) {
                        events::emit(Event::ShowMainWindow);
                    }
                }
            })
            .is_ok(),
        // Lost a race with an instance starting at the same moment.
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => false,
        Err(e) => {
            log::warn!("single-instance lock unavailable: {e}");
            true
        }
    }
}
