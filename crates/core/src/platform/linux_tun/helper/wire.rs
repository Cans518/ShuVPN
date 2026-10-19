//! One bounded local message and at most one borrowed descriptor per operation.
use anyhow::{ensure, Context};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
use tokio::io::unix::AsyncFd;

pub(super) const VERSION: u32 = 2;
const MAX_MESSAGE: usize = 80 * 1024;
const MAX_DESCRIPTORS: usize = 8;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub id: u64,
    pub command: Command,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
pub(super) enum Command {
    Hello {
        version: u32,
    },
    Create {
        plan: String,
    },
    #[cfg(feature = "tun-probe")]
    CreateTransferFailure {
        plan: String,
    },
    Routes {
        hosts: Vec<std::net::IpAddr>,
    },
    BindSocket {
        interface: Option<String>,
    },
    Close {},
    Shutdown {},
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Response {
    pub id: u64,
    pub reply: Reply,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", deny_unknown_fields)]
pub(super) enum Reply {
    Ready { version: u32, privileged: bool },
    Created { document: String },
    Done {},
    Failure { code: String, message: String },
}

pub(super) struct Channel(AsyncFd<OwnedFd>);

impl Channel {
    pub(super) fn pair() -> io::Result<(Self, OwnedFd)> {
        let mut descriptors = [-1; 2];
        // SAFETY: socketpair writes exactly two descriptors into this live array.
        let result = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
                descriptors.as_mut_ptr(),
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful syscall returned two unique, owned descriptors.
        let (parent, child) = unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        };
        Ok((Self(AsyncFd::new(parent)?), child))
    }

    pub(super) fn inherited(descriptor: OwnedFd) -> io::Result<Self> {
        Ok(Self(AsyncFd::new(descriptor)?))
    }

    pub(super) fn process_descriptor(&self) -> anyhow::Result<OwnedFd> {
        // pre_exec queued this frame before spawn could return successfully.
        let received = loop {
            match receive_message(self.0.as_raw_fd()) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        let (bytes, descriptor) = received.context("helper process descriptor missing")?;
        ensure!(bytes == b"pidfd", "invalid helper process descriptor frame");
        descriptor.context("helper process descriptor missing")
    }

    pub(super) fn peer(&self) -> io::Result<libc::ucred> {
        // SAFETY: all-zero is valid for the plain C credential structure.
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut size = std::mem::size_of_val(&credentials) as libc::socklen_t;
        // SAFETY: the writable credential buffer and its length are correctly sized.
        let result = unsafe {
            libc::getsockopt(
                self.0.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut size,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        if size as usize != std::mem::size_of_val(&credentials) {
            return Err(io::Error::other("invalid helper peer credentials"));
        }
        Ok(credentials)
    }

    pub(super) async fn send<T: Serialize>(
        &self,
        value: &T,
        descriptor: Option<&OwnedFd>,
    ) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(value)?;
        ensure!(
            (1..=MAX_MESSAGE).contains(&bytes.len()),
            "helper message too large"
        );
        loop {
            let mut ready = self.0.writable().await?;
            match ready.try_io(|socket| send_message(socket.as_raw_fd(), &bytes, descriptor)) {
                Ok(result) => return result.map_err(Into::into),
                Err(_) => continue,
            }
        }
    }

    pub(super) async fn receive<T: DeserializeOwned>(
        &self,
    ) -> anyhow::Result<Option<(T, Option<OwnedFd>)>> {
        loop {
            let mut ready = self.0.readable().await?;
            let received = match ready.try_io(|socket| receive_message(socket.as_raw_fd())) {
                Ok(result) => result?,
                Err(_) => continue,
            };
            let Some((bytes, descriptor)) = received else {
                return Ok(None);
            };
            return Ok(Some((
                serde_json::from_slice(&bytes).context("invalid helper message")?,
                descriptor,
            )));
        }
    }
}

// Called between fork and exec: this path uses stack buffers and sendmsg only.
pub(super) fn send_process_descriptor(socket: i32, descriptor: &OwnedFd) -> io::Result<()> {
    loop {
        match send_message(socket, b"pidfd", Some(descriptor)) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

fn send_message(socket: i32, bytes: &[u8], descriptor: Option<&OwnedFd>) -> io::Result<()> {
    // usize storage gives cmsghdr its required alignment.
    let mut control = [0usize; 16];
    let mut vector = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: zero initializes unused msghdr members to their documented defaults.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut vector;
    message.msg_iovlen = 1;
    if let Some(descriptor) = descriptor {
        message.msg_control = control.as_mut_ptr().cast();
        // SAFETY: the control array is aligned and larger than one descriptor message.
        unsafe {
            message.msg_controllen = libc::CMSG_SPACE(std::mem::size_of::<i32>() as _) as _;
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as _) as _;
            std::ptr::write_unaligned(
                libc::CMSG_DATA(header).cast::<i32>(),
                descriptor.as_raw_fd(),
            );
        }
    }
    // SAFETY: all message buffers remain live and the descriptor is only borrowed.
    let written = unsafe { libc::sendmsg(socket, &message, libc::MSG_NOSIGNAL) };
    if written < 0 {
        return Err(io::Error::last_os_error());
    }
    if written as usize != bytes.len() {
        return Err(io::Error::other("partial helper message"));
    }
    Ok(())
}

type Received = Option<(Vec<u8>, Option<OwnedFd>)>;
fn receive_message(socket: i32) -> io::Result<Received> {
    let mut bytes = vec![0; MAX_MESSAGE];
    let mut control = [0usize; 16];
    let mut vector = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: zero initializes unused msghdr members; live buffers are assigned below.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut vector;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    // SAFETY: the aligned control buffer has room for this many descriptors.
    message.msg_controllen =
        unsafe { libc::CMSG_SPACE((MAX_DESCRIPTORS * std::mem::size_of::<i32>()) as _) } as _;
    // SAFETY: recvmsg writes within the two declared live buffers; CLOEXEC is atomic.
    let received = unsafe { libc::recvmsg(socket, &mut message, libc::MSG_CMSG_CLOEXEC) };
    if received < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut descriptors = Vec::new();
    let mut valid_control = true;
    // SAFETY: CMSG iteration stays inside the control buffer written by recvmsg.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            let base = libc::CMSG_LEN(0) as usize;
            let length = (*header).cmsg_len;
            if (*header).cmsg_level != libc::SOL_SOCKET
                || (*header).cmsg_type != libc::SCM_RIGHTS
                || length < base
                || !(length - base).is_multiple_of(std::mem::size_of::<i32>())
            {
                valid_control = false;
            } else {
                let count = (length - base) / std::mem::size_of::<i32>();
                for index in 0..count {
                    // Kernel-delivered SCM_RIGHTS descriptors are newly owned,
                    // including those in a frame subsequently rejected below.
                    descriptors.push(OwnedFd::from_raw_fd(std::ptr::read_unaligned(
                        libc::CMSG_DATA(header).cast::<i32>().add(index),
                    )));
                }
            }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    if !valid_control
        || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0
        || descriptors.len() > 1
    {
        return Err(io::Error::other("invalid helper descriptor frame"));
    }
    if received == 0 {
        return Ok(None);
    }
    bytes.truncate(received as usize);
    Ok(Some((bytes, descriptors.pop())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixDatagram;

    #[tokio::test]
    async fn descriptor_transfer_preserves_identity_and_sets_close_on_exec() -> anyhow::Result<()> {
        let (parent, child) = Channel::pair()?;
        let child = Channel::inherited(child)?;
        let (socket, peer) = UnixDatagram::pair()?;
        let descriptor: OwnedFd = socket.into();
        parent.send(&42u32, Some(&descriptor)).await?;
        let (value, received) = child.receive::<u32>().await?.context("frame missing")?;
        let received = received.context("descriptor missing")?;
        ensure!(value == 42, "message changed");
        // SAFETY: F_GETFD only observes this owned descriptor.
        ensure!(
            unsafe { libc::fcntl(received.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC != 0
        );
        drop(descriptor);
        let socket = UnixDatagram::from(received);
        socket.send(b"same socket")?;
        let mut bytes = [0; 32];
        let length = peer.recv(&mut bytes)?;
        ensure!(&bytes[..length] == b"same socket");
        Ok(())
    }

    #[tokio::test]
    async fn invalid_messages_drop_their_transferred_descriptor() -> anyhow::Result<()> {
        let (parent, child) = Channel::pair()?;
        let child = Channel::inherited(child)?;
        let (socket, peer) = UnixDatagram::pair()?;
        let descriptor = socket.into();
        parent
            .send(&serde_json::json!({"op":"Shell"}), Some(&descriptor))
            .await?;
        drop(descriptor);
        ensure!(child.receive::<Request>().await.is_err());
        ensure!(
            peer.send(b"closed").is_err(),
            "invalid message leaked its descriptor"
        );
        Ok(())
    }

    #[tokio::test]
    async fn oversize_and_unknown_fields_are_rejected() -> anyhow::Result<()> {
        let (parent, child) = Channel::pair()?;
        let child = Channel::inherited(child)?;
        ensure!(parent.send(&"x".repeat(MAX_MESSAGE), None).await.is_err());
        parent
            .send(
                &serde_json::json!({
                    "id": 0, "command": {"op":"Close","unexpected":true}
                }),
                None,
            )
            .await?;
        ensure!(child.receive::<Request>().await.is_err());
        Ok(())
    }
}
