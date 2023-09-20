use std::{net::ToSocketAddrs, io::{self}, fs, path::PathBuf, sync::Arc};
use clap::ArgMatches;
use http::{HttpResponse, HttpRequest, RequestType};
use tokio::{io::{AsyncWriteExt, split}, net::{TcpListener, TcpStream}};
use tokio_rustls::{TlsAcceptor, rustls, server::TlsStream};

mod http;
mod tls;

pub async fn start(args: &ArgMatches) -> io::Result<()> {
    // Read the command line arguments:
    let path = args.get_one::<PathBuf>("path").expect("Missing path.");
    let addr = format!("127.0.0.1:{}", args.get_one::<String>("port").expect("Missing addr."))
        .to_socket_addrs()?.next()
        .ok_or_else(|| io::Error::from(io::ErrorKind::AddrNotAvailable))?;

    // Load the tls files, and get notes directory.
    let certs = tls::load_certs(&path.join("./server.crt"))?;
    let key = tls::load_keys(&path.join("./server.key"))?;
    let notes = path.join("./notes");

    let config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let acceptor = TlsAcceptor::from(Arc::new(config));

    // Start listening.
    let listener = TcpListener::bind(&addr).await?;

    println!("Server listening at https://{addr}");

    loop {
        let (stream, _peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let notes = notes.clone();

        // Handle the incoming stream:
        let fut = async move {
            let stream = acceptor.accept(stream).await?;
            
            handle_conn(stream, &notes, "1234").await
        };

        // Print any errors that might've occured.
        tokio::spawn(async move {
            if let Err(err) = fut.await {
                eprintln!("{:?}", err);
            }
        });
    }
}

/// Read a note and return it as a HTTP response.
fn read_note(path: &PathBuf) -> HttpResponse {
    let Ok(file) = fs::read_to_string(path) else {
        return HttpResponse::not_found();
    };
    let mut response = HttpResponse::ok();

    response.text(&file);
    response
}

/// Write a note and return it as a HTTP response.
fn write_note(path: &PathBuf, body: &str) -> HttpResponse {
    if path.to_string_lossy().contains("..") {
        return HttpResponse::err_with_context("'..' is not allowed in note paths.");
    };

    if let Err(_) = fs::write(path, body) {
        HttpResponse::err_with_context("Failed to save note file.")
    } else {
        read_note(path)
    }
}

/// Delete a note.
fn delete_note(path: &PathBuf) -> HttpResponse {
    if let Err(err) = fs::remove_file(path) {
        match err.kind() {
            io::ErrorKind::NotFound => HttpResponse::not_found(),
            _ => HttpResponse::err_with_context(&err.to_string()),
        }
    } else {
        HttpResponse::ok()
    }
}

/// Evaluate an incoming HTTP request.
fn eval_request(request: &HttpRequest, notes_dir: &PathBuf) -> HttpResponse {
    match request.path.as_str() {
        /* Read or Write a note */
        s if s.starts_with("/notes") => {
            let dir = notes_dir.join(format!(".{}", &s[6..]));

            match request.req_type {
                RequestType::GET => read_note(&dir),
                RequestType::POST => write_note(&dir, &request.body),
                RequestType::DELETE => delete_note(&dir),

                _ => HttpResponse::not_found()
            }
        }
        
        /* List all notes */
        s if s.starts_with("/list") => {
            if s.len() <= 6 || !s.contains('?') {
                return HttpResponse::err_with_context("Missing query string '?<start>:<end>'");
            }

            // Bounds checks:
            let mut bounds = s[6..].split(':');
            let Some(start) = bounds.next() else {
                return HttpResponse::err_with_context("Missing start bounds '?<start>:<end>'");
            };
            let Some(end) = bounds.next() else {
                return HttpResponse::err_with_context("Missing end bounds '?<start>:<end>'");
            };

            let Ok(start) = start.parse::<u16>() else {
                return HttpResponse::err_with_context("Start bounds is not a valid number");
            };
            let Ok(end) = end.parse::<u16>() else {
                return HttpResponse::err_with_context("End bounds is not a valid number");
            };

            if start > end {
                return HttpResponse::err_with_context("Start of the bounds is bigger then the end");
            }

            let mut notes: Vec<String> = Vec::new();
            let mut paths = fs::read_dir("notes").unwrap();
            let mut i = 0;

            while let Some(Ok(entry)) = paths.next() {
                if i < start || i >= end {
                    i += 1;
                    continue;
                }

                notes.push(entry.file_name().to_string_lossy().to_string());
                i += 1;
            }

            let mut response = HttpResponse::ok();
            response.json(&format!("[\"{}\"]", notes.join("\", \"")));
            response
        }

        /* Get server status */
        "/status" => HttpResponse::ok(),

        _ => HttpResponse::not_found()
    }
}

/// Handle an incoming connection.
async fn handle_conn(stream: TlsStream<TcpStream>, notes_dir: &PathBuf, auth: &str) -> io::Result<()> {
    let (mut reader, mut writer) = split(stream);

    let req = HttpRequest::parse(&mut reader).await?;

    // Check if the auth header is valid:
    let Some(auth_header) = req.headers.get("Authorization") else {
        return HttpResponse::unauth().send(&mut writer).await;
    };
    if auth_header.trim() != auth.trim() {
        return HttpResponse::unauth().send(&mut writer).await;
    }

    let res = eval_request(&req, &notes_dir);

    res.send(&mut writer).await?;
    writer.shutdown().await
}
