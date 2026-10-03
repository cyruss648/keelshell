use super::{ProxyCredentials, ProxyError};
use crate::Result;
use std::net::IpAddr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

pub(super) async fn connect<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    credentials: Option<&ProxyCredentials>,
    host: &str,
    port: u16,
) -> Result<()> {
    let method = if credentials.is_some() { 2 } else { 0 };
    stream.write_all(&[5, 1, method]).await?;
    stream.flush().await?;
    let mut response = [0; 2];
    stream.read_exact(&mut response).await?;
    if response[0] != 5 {
        return Err(ProxyError::InvalidResponse("SOCKS version").into());
    }
    if response[1] != method {
        return Err(ProxyError::UnsupportedAuthentication.into());
    }
    if let Some(credentials) = credentials {
        let mut auth = Zeroizing::new(Vec::with_capacity(
            3 + credentials.username.len() + credentials.password.len(),
        ));
        auth.extend_from_slice(&[1, credentials.username.len() as u8]);
        auth.extend_from_slice(credentials.username.as_bytes());
        auth.push(credentials.password.len() as u8);
        auth.extend_from_slice(credentials.password.as_bytes());
        stream.write_all(&auth).await?;
        stream.flush().await?;
        drop(auth);
        stream.read_exact(&mut response).await?;
        if response[0] != 1 {
            return Err(ProxyError::InvalidResponse("SOCKS authentication version").into());
        }
        if response[1] != 0 {
            return Err(ProxyError::AuthenticationRejected.into());
        }
    }
    let mut request = vec![5, 1, 0];
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            request.push(1);
            request.extend_from_slice(&ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            request.push(4);
            request.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            request.extend_from_slice(&[3, host.len() as u8]);
            request.extend_from_slice(host.as_bytes());
        }
    }
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;
    stream.flush().await?;
    let mut reply = [0; 4];
    stream.read_exact(&mut reply).await?;
    if reply[0] != 5 || reply[2] != 0 {
        return Err(ProxyError::InvalidResponse("SOCKS reply header").into());
    }
    if reply[1] != 0 {
        return Err(ProxyError::Socks5Rejected(reply[1]).into());
    }
    let count = match reply[3] {
        1 => 4,
        4 => 16,
        3 => {
            let count = stream.read_u8().await?;
            if count == 0 {
                return Err(ProxyError::InvalidResponse("empty SOCKS bound name").into());
            }
            usize::from(count)
        }
        _ => return Err(ProxyError::InvalidResponse("SOCKS address type").into()),
    };
    let mut bound = vec![0; count + 2];
    stream.read_exact(&mut bound).await?;
    Ok(())
}
