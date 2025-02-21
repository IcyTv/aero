use num_traits::cast::FromPrimitive;

use aero_syscall::socket::MessageHeader;
use aero_syscall::*;
use alloc::sync::Arc;

use crate::fs::cache::DirCacheItem;
use crate::fs::inode::DirEntry;
use crate::fs::inode::INodeInterface;
use crate::mem::paging::VirtAddr;

use crate::socket::inet::InetSocket;
use crate::socket::unix::*;
use crate::socket::SocketAddr;

use crate::userland::scheduler;

/// Creates a [`SocketAddr`] from the provided userland socket structure address. This
/// is done by looking at the family field present in every socket address structure.
fn socket_addr_from_addr<'sys>(address: VirtAddr) -> Result<SocketAddr<'sys>, SyscallError> {
    let family = address.read_mut::<u32>()?.clone();
    Ok(SocketAddr::from_family(address, family)?)
}

/// Connects the socket to the specified address.
#[syscall]
pub fn connect(fd: usize, address: usize, length: usize) -> Result<usize, SyscallError> {
    let address = socket_addr_from_addr(VirtAddr::new(address as u64))?;
    let file = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EINVAL)?;

    file.inode().connect(address, length)?;
    Ok(0)
}

/// Accept a connection on a socket.
#[syscall]
pub fn accept(fd: usize, address: usize, length: usize) -> Result<usize, SyscallError> {
    let file_table = scheduler::get_scheduler().current_task().file_table.clone();
    let socket = file_table.get_handle(fd).ok_or(SyscallError::EINVAL)?;

    let address = if address != 0 && length != 0 {
        Some((
            VirtAddr::new(address as u64),
            VirtAddr::new(length as u64).read_mut::<u32>()?,
        ))
    } else {
        None
    };

    let connection_sock = socket.inode().accept(address)?;
    let handle = file_table.open_file(
        DirEntry::from_inode(connection_sock, String::from("<socket>")),
        OpenFlags::O_RDWR,
    )?;

    Ok(handle)
}

#[syscall]
pub fn sock_send(
    fd: usize,
    header: &mut MessageHeader,
    flags: usize,
) -> Result<usize, SyscallError> {
    assert!(flags == 0, "sock_send: flags are not currently supported");

    let current_task = scheduler::get_scheduler().current_task();
    let socket = current_task
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EINVAL)?;

    Ok(socket.inode().send(header)?)
}

#[syscall]
pub fn sock_recv(
    sockfd: usize,
    header: &mut MessageHeader,
    flags: usize,
) -> Result<usize, SyscallError> {
    assert!(flags == 0, "sock_recv: flags are not currently supported");

    let current_task = scheduler::get_scheduler().current_task();
    let socket = current_task
        .file_table
        .get_handle(sockfd)
        .ok_or(SyscallError::EINVAL)?;

    Ok(socket.inode().recv(header)?)
}

/// Marks the socket as a passive socket (i.e. as a socket that will be used to accept incoming
/// connection requests).
#[syscall]
pub fn listen(fd: usize, backlog: usize) -> Result<usize, SyscallError> {
    let file = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EINVAL)?;

    file.inode().listen(backlog)?;
    Ok(0)
}

fn create_socket(
    domain: usize,
    socket_type: usize,
    protocol: usize,
) -> Result<DirCacheItem, SyscallError> {
    let typ = SocketType::from_usize(socket_type).ok_or(SyscallError::EINVAL)?;
    let protocol = IpProtocol::from_usize(protocol).ok_or(SyscallError::EINVAL)?;

    let socket = match domain as u32 {
        AF_UNIX => UnixSocket::new() as Arc<dyn INodeInterface>,
        AF_INET => InetSocket::new(typ, protocol)? as Arc<dyn INodeInterface>,

        _ => {
            log::warn!(
                "unsupported socket type: domain={domain}, socket_type={socket_type}, protocol={protocol:?}"
            );

            return Err(SyscallError::EINVAL);
        }
    };

    let entry = DirEntry::from_inode(socket, String::from("<socket>"));
    Ok(entry)
}

#[syscall]
pub fn socket(domain: usize, socket_type: usize, protocol: usize) -> Result<usize, SyscallError> {
    let entry = create_socket(domain, socket_type, protocol)?;

    let current_task = scheduler::get_scheduler().current_task();

    let sockfd_flags = SocketFlags::from_bits_truncate(socket_type).into();
    let fd = current_task.file_table.open_file(entry, sockfd_flags)?;

    Ok(fd)
}

#[syscall]
pub fn bind(fd: usize, address: usize, length: usize) -> Result<usize, SyscallError> {
    let address = socket_addr_from_addr(VirtAddr::new(address as u64))?;

    let current_task = scheduler::get_scheduler().current_task();
    let file = current_task.file_table.get_handle(fd);

    match file {
        Some(handle) => {
            if handle.inode().metadata()?.is_socket() {
                handle.inode().bind(address, length)?;

                Ok(0)
            } else {
                Err(SyscallError::ENOTSOCK)
            }
        }
        None => Err(SyscallError::ENOENT),
    }
}

/// Create an unbound pair of connected sockets in a specified domain, of a
/// specified type, under the protocol optionally specified by the protocol
/// argument. The two sockets shall be identical. The file descriptors used
/// in referencing the created sockets shall be returned in fds[0] and fds[1].
#[syscall]
pub fn socket_pair(
    domain: usize,
    type_and_flags: usize,
    protocol: usize,
    fds: &mut [i32; 2],
) -> Result<usize, SyscallError> {
    let current_task = scheduler::get_scheduler().current_task();
    let sockfd_flags = SocketFlags::from_bits_truncate(type_and_flags).into();

    let a = create_socket(domain, type_and_flags, protocol)?;
    let b = create_socket(domain, type_and_flags, protocol)?;

    UnixSocket::connect_pair(a.clone(), b.clone())?;

    fds[0] = current_task.file_table.open_file(a, sockfd_flags)? as i32;
    fds[1] = current_task.file_table.open_file(b, sockfd_flags)? as i32;
    Ok(0)
}
