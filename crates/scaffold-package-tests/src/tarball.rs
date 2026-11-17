//! A small writer of tar files (the ustar format), so that a test can make the members a real `tar` would refuse to make or would change: a name with
//! `..` or a newline, a symlink, a hardlink, a pipe, two members with one name.

pub enum Kind {
    File,
    Dir,
    Symlink,
    Hardlink,
    Fifo,
}

pub struct Member {
    pub name: String,
    pub kind: Kind,
    pub content: Vec<u8>,
    /// The target of a symlink or hardlink.
    pub link: String,
}

impl Member {
    pub fn file(name: &str, content: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: Kind::File,
            content: content.as_bytes().to_vec(),
            link: String::new(),
        }
    }

    pub fn dir(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: Kind::Dir,
            content: vec![],
            link: String::new(),
        }
    }

    pub fn link(name: &str, kind: Kind, target: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            content: vec![],
            link: target.to_owned(),
        }
    }
}

fn put(header: &mut [u8; 512], offset: usize, length: usize, text: &[u8]) {
    assert!(text.len() <= length, "a field is too long: {text:?}");
    header[offset..offset + text.len()].copy_from_slice(text);
}

fn octal(value: usize, digits: usize) -> Vec<u8> {
    format!("{value:0digits$o}").into_bytes()
}

fn header(member: &Member) -> [u8; 512] {
    let mut h = [0u8; 512];
    put(&mut h, 0, 100, member.name.as_bytes());
    let (mode, flag) = match member.kind {
        Kind::File => (0o644, b'0'),
        Kind::Dir => (0o755, b'5'),
        Kind::Symlink => (0o777, b'2'),
        Kind::Hardlink => (0o644, b'1'),
        Kind::Fifo => (0o644, b'6'),
    };
    put(&mut h, 100, 8, &[octal(mode, 7), vec![0]].concat());
    put(&mut h, 108, 8, &[octal(0, 7), vec![0]].concat());
    put(&mut h, 116, 8, &[octal(0, 7), vec![0]].concat());
    let size = if matches!(member.kind, Kind::File) {
        member.content.len()
    } else {
        0
    };
    put(&mut h, 124, 12, &[octal(size, 11), vec![0]].concat());
    put(
        &mut h,
        136,
        12,
        &[octal(1_700_000_000, 11), vec![0]].concat(),
    );
    h[148..156].copy_from_slice(b"        ");
    h[156] = flag;
    put(&mut h, 157, 100, member.link.as_bytes());
    put(&mut h, 257, 6, b"ustar\0");
    put(&mut h, 263, 2, b"00");
    let sum: usize = h.iter().map(|b| usize::from(*b)).sum();
    put(&mut h, 148, 8, &[octal(sum, 6), vec![0, b' ']].concat());
    h
}

/// The bytes of a tar file with these members, in this order.
pub fn build(members: &[Member]) -> Vec<u8> {
    let mut out = Vec::new();
    for member in members {
        out.extend_from_slice(&header(member));
        if matches!(member.kind, Kind::File) {
            out.extend_from_slice(&member.content);
            out.resize(out.len().div_ceil(512) * 512, 0);
        }
    }
    out.extend_from_slice(&[0u8; 1024]);
    out
}
