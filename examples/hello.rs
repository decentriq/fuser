use clap::{crate_version, Arg, Command};
use fuser::{FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request, KernelConfig, TimeOrNow, ReplyEmpty, ReplyOpen, ReplyWrite, ReplyDirectoryPlus, ReplyStatfs, ReplyXattr, ReplyCreate, ReplyLock, ReplyBmap, ReplyIoctl, ReplyLseek};
use libc::ENOENT;
use std::ffi::OsStr;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1); // 1 second

const HELLO_DIR_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

const HELLO_TXT_ATTR: FileAttr = FileAttr {
    ino: 2,
    size: 13,
    blocks: 1,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::RegularFile,
    perm: 0o644,
    nlink: 1,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

struct HelloFS {
    fh: u64,
    hello_txt_content: Vec<u8>,
}

impl HelloFS {
    fn hello_txt_attr(&self) -> FileAttr {
        let mut attr = HELLO_TXT_ATTR;
        attr.size = self.hello_txt_content.len() as u64;
        let time = SystemTime::now();
        attr.atime = time;
        attr.mtime = time;
        attr.ctime = time;
        attr
    }

    fn next_fh(&mut self) -> u64 {
        let fh = self.fh;
        self.fh += 1;
        fh
    }
}

impl Filesystem for HelloFS {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 && name.to_str() == Some("hello.txt") {
            let attr = self.hello_txt_attr();
            reply.entry(&TTL, &attr, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &HELLO_DIR_ATTR),
            2 => {
                let attrs = self.hello_txt_attr();
                log::info!("getattr => {:?}", attrs);
                reply.attr(&TTL, &attrs)
            },
            _ => reply.error(ENOENT),
        }
    }

    fn setattr(&mut self, _req: &Request<'_>, ino: u64, mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, _atime: Option<TimeOrNow>, _mtime: Option<TimeOrNow>, _ctime: Option<SystemTime>, fh: Option<u64>, _crtime: Option<SystemTime>, _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>, flags: Option<u32>, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &HELLO_DIR_ATTR),
            2 => reply.attr(&TTL, &self.hello_txt_attr()),
            _ => reply.error(ENOENT),
        }
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: ReplyOpen) {
        if ino == 2 {
            reply.opened(self.next_fh(), flags as u32)
        } else {
            reply.error(ENOENT);
        }
    }

    fn read(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        if ino == 2 {
            let offset = offset as usize;
            let size = size as usize;
            let read_size = size.min(self.hello_txt_content.len() - offset);
            let read = &self.hello_txt_content.as_slice()[offset .. offset + read_size];
            log::info!("Read: {:?}", read);
            reply.data(read);
        } else {
            reply.error(ENOENT);
        }
    }

    fn write(&mut self, _req: &Request<'_>, ino: u64, fh: u64, offset: i64, data: &[u8], write_flags: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyWrite) {
         if ino == 2 {
            let offset = offset as usize;
            let overwrite_len = data.len().min(self.hello_txt_content.len() - offset);
            self.hello_txt_content.as_mut_slice()[offset .. offset + overwrite_len].copy_from_slice(&data[.. overwrite_len]);
            self.hello_txt_content.extend_from_slice(&data[overwrite_len ..]);
             log::info!("Contents: {:?}", self.hello_txt_content);
            reply.written(data.len() as u32)
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
            (2, FileType::RegularFile, "hello.txt"),
        ];

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}

fn main() {
    let matches = Command::new("hello")
        .version(crate_version!())
        .author("Christopher Berner")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(true)
                .index(1)
                .help("Act as a client, and mount FUSE at given path"),
        )
        .arg(
            Arg::new("auto_unmount")
                .long("auto_unmount")
                .help("Automatically unmount on process exit"),
        )
        .arg(
            Arg::new("allow-root")
                .long("allow-root")
                .help("Allow root user to access filesystem"),
        )
        .get_matches();
    env_logger::init();
    let mountpoint = matches.value_of("MOUNT_POINT").unwrap();
    let mut options = vec![MountOption::RW, MountOption::FSName("hello".to_string())];
    if matches.is_present("auto_unmount") {
        options.push(MountOption::AutoUnmount);
    }
    if matches.is_present("allow-root") {
        options.push(MountOption::AllowRoot);
    }
    fuser::mount2(HelloFS { fh: 0, hello_txt_content: vec![] }, mountpoint, &options).unwrap();
}

#[test]
fn stale_data_bug() {
    use std::io::Read;
    use std::io::Write;
    let mut file_read = std::fs::File::open("/tmp/mnt/hello.txt").unwrap();
    let mut file_write = std::fs::File::create("/tmp/mnt/hello.txt").unwrap();
    file_write.write_all("Init".as_bytes()).unwrap();
    let mut buffer1 = vec![];
    file_read.read_to_end(&mut buffer1).unwrap();
    println!("buffer1 {:?}", buffer1);
    file_write.write_all("Hello World!".as_bytes()).unwrap();
    let mut buffer2 = vec![];
    file_read.read_to_end(&mut buffer2).unwrap();
    println!("buffer2 {:?}", buffer2);
}
