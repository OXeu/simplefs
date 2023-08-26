use libc::c_int;
pub type ErrorCode = c_int;
#[allow(unused)]
// EPERM: Operation not permitted
pub const EPERM: c_int = 1;
#[allow(unused)]
// ENOENT: No such file or directory
pub const ENOENT: c_int = 2;
#[allow(unused)]
// ESRCH: No such process
pub const ESRCH: c_int = 3;
#[allow(unused)]
// EINTR: Interrupted system call
pub const EINTR: c_int = 4;
#[allow(unused)]
// EIO: Input/output error
pub const EIO: c_int = 5;
#[allow(unused)]
// ENXIO: No such device or address
pub const ENXIO: c_int = 6;
#[allow(unused)]
// E2BIG: Argument list too long
pub const E2BIG: c_int = 7;
#[allow(unused)]
// ENOEXEC: Exec format error
pub const ENOEXEC: c_int = 8;
#[allow(unused)]
// EBADF: Bad file descriptor
pub const EBADF: c_int = 9;
#[allow(unused)]
// ECHILD: No child processes
pub const ECHILD: c_int = 10;
#[allow(unused)]
// EAGAIN: Resource temporarily unavailable
pub const EAGAIN: c_int = 11;
#[allow(unused)]
// ENOMEM: Cannot allocate memory
pub const ENOMEM: c_int = 12;
#[allow(unused)]
// EACCES: Permission denied
pub const EACCES: c_int = 13;
#[allow(unused)]
// EFAULT: Bad address
pub const EFAULT: c_int = 14;
#[allow(unused)]
// ENOTBLK: Block device required
pub const ENOTBLK: c_int = 15;
#[allow(unused)]
// EBUSY: Device or resource busy
pub const EBUSY: c_int = 16;
#[allow(unused)]
// EEXIST: File exists
pub const EEXIST: c_int = 17;
#[allow(unused)]
// EXDEV: Invalid cross-device link
pub const EXDEV: c_int = 18;
#[allow(unused)]
// ENODEV: No such device
pub const ENODEV: c_int = 19;
#[allow(unused)]
// ENOTDIR: Not a directory
pub const ENOTDIR: c_int = 20;
#[allow(unused)]
// EISDIR: Is a directory
pub const EISDIR: c_int = 21;
#[allow(unused)]
// EINVAL: Invalid argument
pub const EINVAL: c_int = 22;
#[allow(unused)]
// ENFILE: Too many open files in system
pub const ENFILE: c_int = 23;
#[allow(unused)]
// EMFILE: Too many open files
pub const EMFILE: c_int = 24;
#[allow(unused)]
// ENOTTY: Inappropriate ioctl for device
pub const ENOTTY: c_int = 25;
#[allow(unused)]
// ETXTBSY: Text file busy
pub const ETXTBSY: c_int = 26;
#[allow(unused)]
// EFBIG: File too large
pub const EFBIG: c_int = 27;
#[allow(unused)]
// ENOSPC: No space left on device
pub const ENOSPC: c_int = 28;
#[allow(unused)]
// ESPIPE: Illegal seek
pub const ESPIPE: c_int = 29;
#[allow(unused)]
// EROFS: Read-only file system
pub const EROFS: c_int = 30;
#[allow(unused)]
// EMLINK: Too many links
pub const EMLINK: c_int = 31;
#[allow(unused)]
// EPIPE: Broken pipe
pub const EPIPE: c_int = 32;
#[allow(unused)]
// EDOM: Numerical argument out of domain
pub const EDOM: c_int = 33;
#[allow(unused)]
// ERANGE: Result too large
pub const ERANGE: c_int = 34;
#[allow(unused)]
// EWOULDBLOCK: Operation would block
pub const EWOULDBLOCK: c_int = EAGAIN;
