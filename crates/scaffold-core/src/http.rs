//! A plain HTTPS GET that returns the body as text, over `ureq`. The runner's network commands (`freshness`, `latest`,
//! `upgrade`) use it and add their own retries on top.

use std::time::Duration;

/// Fetches `url` and returns the body. Redirects are followed. A status outside 200 to 299 is the error `HTTP <status>`;
/// anything else that goes wrong is the error text of the connection. The whole request, body included, is stopped
/// after `timeout`.
pub fn get_text(url: &str, timeout: Duration) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // A status is reported as `HTTP <status>` below, the way the TypeScript runner reports it.
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serves each canned response once, in order, on a local port, and returns the address. The server answers each
    /// connection and closes it. `None` as a response means "read the request and never answer".
    fn serve(responses: Vec<Option<(u16, &'static str, &'static str)>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut seen = Vec::new();
                let mut buffer = [0u8; 1024];
                while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => seen.extend_from_slice(&buffer[..n]),
                    }
                }
                match response {
                    Some((status, extra, body)) => {
                        let text = format!(
                            "HTTP/1.1 {status} X\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(text.as_bytes());
                    }
                    None => std::thread::sleep(Duration::from_secs(5)),
                }
            }
        });
        format!("http://{address}")
    }

    #[test]
    fn a_200_returns_the_body_as_text() {
        let base = serve(vec![Some((200, "", "hello é"))]);
        assert_eq!(
            get_text(&format!("{base}/x"), Duration::from_secs(5)),
            Ok("hello é".to_owned())
        );
    }

    #[test]
    fn every_status_outside_200_to_299_is_an_error_naming_it() {
        for status in [301u16, 404, 429, 500, 503] {
            let extra = if status == 301 {
                "Location: /missing-target\r\n"
            } else {
                ""
            };
            let base = serve(vec![Some((status, extra, "no"))]);
            let result = get_text(&format!("{base}/x"), Duration::from_secs(5));
            // A 301 without a usable target is reported as an HTTP error too, not followed forever.
            assert!(result.is_err(), "{status}: {result:?}");
            if status != 301 {
                assert_eq!(result, Err(format!("HTTP {status}")));
            }
        }
    }

    #[test]
    fn a_204_and_a_299_style_success_are_accepted() {
        let base = serve(vec![Some((204, "", ""))]);
        assert_eq!(
            get_text(&format!("{base}/x"), Duration::from_secs(5)),
            Ok(String::new())
        );
    }

    #[test]
    fn a_redirect_is_followed() {
        let base = serve(vec![
            Some((302, "Location: /final\r\n", "")),
            Some((200, "", "arrived")),
        ]);
        assert_eq!(
            get_text(&format!("{base}/start"), Duration::from_secs(5)),
            Ok("arrived".to_owned())
        );
    }

    #[test]
    fn a_connection_that_is_refused_is_an_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(
            get_text(
                &format!("http://127.0.0.1:{port}/x"),
                Duration::from_secs(5)
            )
            .is_err()
        );
    }

    #[test]
    fn a_server_that_never_answers_is_stopped_at_the_timeout() {
        let base = serve(vec![None]);
        let started = std::time::Instant::now();
        let result = get_text(&format!("{base}/x"), Duration::from_millis(400));
        assert!(result.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_url_that_is_not_a_url_is_an_error() {
        assert!(get_text("not a url", Duration::from_secs(1)).is_err());
    }
}
