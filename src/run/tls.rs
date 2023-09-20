use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use tokio_rustls::rustls::{PrivateKey, Certificate};

pub fn load_certs(path: &Path) -> io::Result<Vec<Certificate>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)?;

    Ok(certs.into_iter().map(Certificate).collect())
}

pub fn load_keys(path: &Path) -> io::Result<PrivateKey> {
    // Open keyfile.
    let keyfile = File::open(path)?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    match rustls_pemfile::read_one(&mut reader)? {
        Some(rustls_pemfile::Item::PKCS8Key(key)) => Ok(PrivateKey(key)),
        _ => Err(io::Error::new(
            io::ErrorKind::Other,
            "Private key has to be the first entry in the key file.".to_string(),
        )),
    }
}
