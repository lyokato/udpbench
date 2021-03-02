use thiserror::Error;

#[derive(Error, Debug)]
pub enum ErrorKind {
    #[error("failed to bind socket")]
    SocketBindFailure(#[source] std::io::Error),
    #[error("failed to build socket")]
    SocketBuildFailure(#[source] std::io::Error),
    #[error("socket address has bad format")]
    BadAddress,
    #[error("failed to set SO_REUSEADDR socket option")]
    CantSetOptionReuseAddress(#[source] std::io::Error),
    #[error("failed to set SO_REUSEPORT socket option")]
    CantSetOptionReusePort(#[source] std::io::Error),
    #[error("failed to set 'nonblocking' socket option")]
    CantSetOptionNonBlocking(#[source] std::io::Error),
    #[error("failed to set 'SO_ATTACH_REUSEPORT_CBPF' socket option")]
    CantSetOptionAttachReusePortCbpf,
    #[error("method is called on bad state")]
    BadClusterState,
}

pub type Result<T> = std::result::Result<T, ErrorKind>;
