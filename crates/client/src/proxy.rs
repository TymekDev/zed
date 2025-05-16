//! client proxy

mod http_proxy;
mod socks_proxy;

use anyhow::{Context, Result, anyhow};
use http_client::Url;
use socks_proxy::{Socks4Identification, Socks5Authorization, SocksVersion};
use tokio_socks::tcp::{Socks4Stream, Socks5Stream};

pub(crate) async fn connect_with_proxy_stream(
    proxy: &Url,
    rpc_host: (&str, u16),
) -> Result<Box<dyn AsyncReadWrite>> {
    println!(
        "Connecting to socks proxy: {:?}, with ({:?})",
        proxy, rpc_host
    );
    let Some((socks_proxy, version)) = parse_socks_proxy(proxy) else {
        // If parsing the proxy URL fails, we must avoid falling back to an insecure connection.
        // SOCKS proxies are often used in contexts where security and privacy are critical,
        // so any fallback could expose users to significant risks.
        return Err(anyhow!("Parsing proxy url failed"));
    };

    // Connect to proxy and wrap protocol later
    let stream = tokio::net::TcpStream::connect(socks_proxy)
        .await
        .context("Failed to connect to socks proxy")?;

    let socks: Box<dyn AsyncReadWrite> = match version {
        SocksVersion::V4(None) => {
            let socks = Socks4Stream::connect_with_socket(stream, rpc_host)
                .await
                .context("error connecting to socks")?;
            Box::new(socks)
        }
        SocksVersion::V4(Some(Socks4Identification { user_id })) => {
            let socks = Socks4Stream::connect_with_userid_and_socket(stream, rpc_host, user_id)
                .await
                .context("error connecting to socks")?;
            Box::new(socks)
        }
        SocksVersion::V5(None) => {
            let socks = Socks5Stream::connect_with_socket(stream, rpc_host)
                .await
                .context("error connecting to socks")?;
            Box::new(socks)
        }
        SocksVersion::V5(Some(Socks5Authorization { username, password })) => {
            let socks = Socks5Stream::connect_with_password_and_socket(
                stream, rpc_host, username, password,
            )
            .await
            .context("error connecting to socks")?;
            Box::new(socks)
        }
    };

    Ok(socks)
}

pub(crate) trait AsyncReadWrite:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static
{
}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static> AsyncReadWrite
    for T
{
}

#[cfg(test)]
mod tests {
    use url::Url;

    use crate::connect_with_proxy_stream;

    /// If parsing the proxy URL fails, we must avoid falling back to an insecure connection.
    /// SOCKS proxies are often used in contexts where security and privacy are critical,
    /// so any fallback could expose users to significant risks.
    #[tokio::test]
    async fn fails_on_bad_proxy() {
        // Should fail connecting because http is not a valid Socks proxy scheme
        let proxy = Url::parse("http://localhost:2313").unwrap();

        let result = connect_with_proxy_stream(&proxy, ("test", 1080)).await;
        match result {
            Err(e) => assert_eq!(e.to_string(), "Parsing proxy url failed"),
            Ok(_) => panic!("Connecting on bad proxy should fail"),
        };
    }
}
