use std::{
    io, collections::HashMap
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

type TlsReader = tokio::io::ReadHalf<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>;
type TlsWriter = tokio::io::WriteHalf<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>;

/// HTTP response builder.
pub(crate) struct HttpResponse {
    status: String,
    content: Option<String>,
    content_type: String,
}

impl HttpResponse {
    /// Create a new 200 OK response.
    pub fn ok() -> Self {
        HttpResponse {
            status: "HTTP/1.1 200 OK\r\n".to_owned(),
            content: None,
            content_type: "text/plain".to_owned(),
        }
    }

    /// Create a new 404 Not Found response.
    pub fn not_found() -> Self {
        HttpResponse {
            status: "HTTP/1.1 404 Not Found\r\n".to_owned(),
            content: Some("404 Not Found".to_owned()),
            content_type: "text/plain".to_owned(),
        }
    }

    /// Create a new 500 Internal Server Error response.
    #[allow(dead_code)]
    pub fn err() -> Self {
        HttpResponse {
            status: "HTTP/1.1 500 Internal Server Error\r\n".to_owned(),
            content: Some("500 Internal Server Error".to_owned()),
            content_type: "text/plain".to_owned(),
        }
    }

    /// Create a new 401 Unauthorized response.
    pub fn unauth() -> Self {
        HttpResponse {
            status: "HTTP/1.1 401 Unauthorized\r\n".to_owned(),
            content: Some("401 Unauthorized".to_owned()),
            content_type: "text/plain".to_owned(),
        }
    }

    /// Create a new 500 Internal Server Error response.
    pub fn err_with_context(context: &str) -> Self {
        HttpResponse {
            status: "HTTP/1.1 500 Internal Server Error\r\n".to_owned(),
            content: Some(format!("500 Internal Server Error\r\n\r\n{}", context)),
            content_type: "text/plain".to_owned(),
        }
    }

    /// Add text content to the response.
    pub fn text(&mut self, content: &str) -> &Self {
        self.content = Some(content.to_owned());
        self
    }

    /// Add html content to the response.
    #[allow(dead_code)]
    pub fn html(&mut self, content: &str) -> &Self {
        self.content = Some(content.to_owned());
        self.content_type = "text/html".to_owned();
        self
    }

    /// Add json content to the response.
    pub fn json(&mut self, content: &str) -> &Self {
        self.content = Some(content.to_owned());
        self.content_type = "application/json".to_owned();
        self
    }

    /// Send the HTTP response over Tcp.
    pub async fn send(&self, stream: &mut TlsWriter) -> io::Result<()> {
        let mut response = self.status.clone();

        if let Some(content) = &self.content {
            response.push_str(
                format!(
                    "Server: {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                    "mxcop@note-server",
                    content.len(),
                    self.content_type,
                    content
                )
                .as_str(),
            );
        } else {
            response.push_str("Server: mxcop@note-server\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\n\r\n");
        }

        stream.write_all(response.as_bytes()).await?;
        stream.flush().await
    }
}

#[derive(Debug, Default)]
pub(crate) enum RequestType {
    #[default] 
    UNKNOWN, 
    GET, 
    POST, 
    DELETE
}

/// HTTP response builder.
#[derive(Debug, Default)]
pub(crate) struct HttpRequest {
    pub req_type: RequestType,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl HttpRequest {
    pub async fn parse(stream: &mut TlsReader) -> io::Result<Self> {
        // Store all the bytes for our received String
        let mut buf: Vec<u8> = vec![];

        // Read all bytes from the TCP stream:
        let mut rx_bytes = [0u8; 256];
        loop {
            let bytes_read = stream.read(&mut rx_bytes).await?;

            buf.extend_from_slice(&rx_bytes[..bytes_read]);

            if bytes_read < 256 {
                break;
            }
        }
        let buf_len = buf.len();

        // Check if the content is in UTF8.
        let Ok(content) = String::from_utf8(buf.to_vec()) else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "HTTP request doesn't contain valid UTF8"));
        };

        let mut offset = 0;
        let mut request = Self::default();
        let mut first_line = true;

        for line in content.split('\n') {
            // Parse the first line:  "GET /home.html HTTP/1.1"
            if first_line {
                request.req_type = match line {
                    s if s.starts_with("GET") => RequestType::GET,
                    s if s.starts_with("POST") => RequestType::POST,
                    s if s.starts_with("DELETE") => RequestType::DELETE,
                    _ => RequestType::UNKNOWN
                };
                let mut parts = line.split(' ');
                parts.next();
                request.path = parts.next().unwrap_or("/").to_owned();

                first_line = false;
            }

            // Count the offset until we reach the body:
            offset += line.len() + 1;

            if line.len() <= 1 {
                break;
            }

            // Add the headers:
            let Some(header) = line.split_once(':') else {
                continue;
            };
            request.headers.insert(header.0.to_owned(), header.1.to_owned());
        }

        // Grab the body from the request.
        request.body = String::from_utf8(buf[offset..buf_len].to_vec()).unwrap();

        Ok(request)
    }
}
