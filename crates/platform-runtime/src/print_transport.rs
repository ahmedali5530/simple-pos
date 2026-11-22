//! Byte transport for printers. Receipt layout stays in the printing plugin.

use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub async fn send_tcp(address: &str, payload: &[u8]) -> Result<(), String> {
    let mut stream = tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(address))
        .await
        .map_err(|_| format!("Printer {address} did not answer."))?
        .map_err(|err| format!("Printer {address} refused the connection: {err}"))?;
    stream
        .write_all(payload)
        .await
        .map_err(|err| format!("Could not write to {address}: {err}"))?;
    Ok(())
}
