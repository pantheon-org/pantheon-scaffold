//! A local web server standing in for GitHub, so a package built here can be loaded from its package URI.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

/// A server that answers every request with the file of that name (the last segment of the path, percent-decoded) in a folder, or a 404.
/// It runs for as long as the process does.
pub struct Server {
    pub port: u16,
}

pub fn serve_folder(folder: &Path) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("an address").port();
    let folder = folder.to_owned();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let folder = folder.clone();
            std::thread::spawn(move || answer(stream, &folder));
        }
    });
    Server { port }
}

fn percent_decode(text: &str) -> String {
    let hex = |b: u8| char::from(b).to_digit(16);
    let mut out = Vec::new();
    let mut rest = text.as_bytes().iter();
    while let Some(byte) = rest.next() {
        let escaped = match rest.as_slice() {
            [high, low, ..] if *byte == b'%' => hex(*high).zip(hex(*low)),
            _ => None,
        };
        if let Some((high, low)) = escaped {
            out.push((high * 16 + low) as u8);
            rest.nth(1);
        } else {
            out.push(*byte);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn answer(mut stream: TcpStream, folder: &Path) {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => request.extend_from_slice(&buffer[..n]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let target = request
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    let name = percent_decode(path.rsplit('/').next().unwrap_or(""));
    let file: PathBuf = folder.join(name);
    let (status, body) = match (file.is_file(), std::fs::read(&file)) {
        (true, Ok(bytes)) => ("200 OK", bytes),
        _ => ("404 Not Found", Vec::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
}

/// A server that answers a request for a path with the status and body listed for it, and a 500 for any other path. It stands in for the GitHub API
/// for the tests of the pin update. It runs for as long as the process does.
pub fn serve_routes(routes: std::collections::BTreeMap<String, (u16, String)>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("an address").port();
    let routes = std::sync::Arc::new(routes);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let routes = std::sync::Arc::clone(&routes);
            std::thread::spawn(move || answer_route(stream, &routes));
        }
    });
    Server { port }
}

fn answer_route(mut stream: TcpStream, routes: &std::collections::BTreeMap<String, (u16, String)>) {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => request.extend_from_slice(&buffer[..n]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let path = request
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .unwrap_or("/");
    let (status, body) = routes
        .get(path)
        .cloned()
        .unwrap_or((500, format!("no route for {path}")));
    let text = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(text.as_bytes());
}
