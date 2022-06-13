use command_limits::{CommandBuilder, Error as LimitError};

use std::{
    env,
    ffi::OsString,
    io::{self, BufRead},
};

fn bytes_to_os(bytes: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(bytes).to_os_string()
    }
    #[cfg(not(unix))]
    {
        String::from_utf8_lossy(bytes).to_string().into()
    }
}

// If this doesn't make you want to use -0 nothing will
fn read_like_xargs<T: BufRead>(reader: &mut T) -> Option<io::Result<Vec<u8>>> {
    let mut item = vec![];
    let mut complete = false;
    let mut escape = false;
    let mut single = false;
    let mut double = false;
    let mut consumed = 0;

    while !complete {
        {
            let buffer = reader.fill_buf();
            if let Err(e) = buffer {
                return Some(Err(e));
            }
            let buffer = buffer.unwrap();
            if buffer.is_empty() {
                if single || double {
                    return Some(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unterminated quote",
                    )));
                } else if escape {
                    return Some(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "backslash at EOF",
                    )));
                } else if item.is_empty() {
                    return None;
                }
                break;
            }

            for byte in buffer {
                consumed += 1;
                if escape {
                    escape = false;
                    item.push(*byte);
                } else if single {
                    match byte {
                        b'\'' => {
                            single = false;
                        }
                        b'\n' => {
                            return Some(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "unterminated quote",
                            )));
                        }
                        _ => {
                            item.push(*byte);
                        }
                    }
                } else if double {
                    match byte {
                        b'"' => {
                            double = false;
                        }
                        b'\n' => {
                            return Some(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "unterminated quote",
                            )));
                        }
                        _ => {
                            item.push(*byte);
                        }
                    }
                } else {
                    match byte {
                        b'\\' => {
                            escape = true;
                        }
                        b'\'' => {
                            single = true;
                        }
                        b'"' => {
                            double = true;
                        }
                        _ if byte.is_ascii_whitespace() => {
                            complete = !item.is_empty();
                        }
                        _ => {
                            item.push(*byte);
                        }
                    }
                }
                if complete {
                    break;
                }
            }
        }
        reader.consume(consumed);
    }

    return Some(Ok(item));
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("{:?}", e);
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let mut lflag = false;
    let mut oflag = false;
    let mut vflag = false;
    let mut inflags = true;

    let mut command = vec![];

    for arg in env::args_os().skip(1) {
        match arg.to_str() {
            Some("-l" | "--show-limits") if inflags => {
                lflag = true;
            }
            Some("-0" | "--null") if inflags => {
                oflag = true;
            }
            Some("-t" | "--verbose") if inflags => {
                vflag = true;
            }
            Some("--") if inflags => {
                inflags = false;
            }
            Some(f) if inflags && f.starts_with('-') => {
                eprintln!("xargs: unrecognized option `{}'", f);
                eprintln!(concat!(
                    "usage: xargs [-l | --show-limits] [-0 | --null]\n",
                    "             [-t | --verbose] [utility [argument ...]]"
                ));
                std::process::exit(1);
            }
            _ => {
                inflags = false;
                command.push(arg);
            }
        }
    }

    if command.is_empty() {
        command.push("/bin/echo".into());
    }

    let mut basecmd = CommandBuilder::new(&command[0])?;
    if command.len() > 1 {
        basecmd.args(&command[1..])?;
    }

    if lflag {
        let limits = basecmd.get_limits();
        eprintln!("Available argument space: {}", limits.arg_size);
        if let Some(size) = limits.env_size {
            eprintln!("Available environment space: {}", size);
        }
        eprintln!("Space used by initial arguments: {}", basecmd.arg_size());
        eprintln!("Space used by environment: {}", basecmd.env_size());
    }

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut iter: Box<dyn Iterator<Item = io::Result<Vec<u8>>>> = if oflag {
        Box::new(stdin.split(b'\0').fuse())
    } else {
        Box::new(std::iter::from_fn(|| read_like_xargs(&mut stdin)).fuse())
    };

    let mut item = None;

    let mut run_now = false;
    let mut pending = false;

    let mut rc = 0;
    let mut cmd = basecmd.clone();

    loop {
        if item.is_none() {
            item = iter.next().transpose()?;
            if !pending && item.is_none() {
                break;
            }
        }

        if let Some(it) = item.take() {
            if it.is_empty() {
                continue;
            }
            match cmd.arg(bytes_to_os(&it)) {
                Err(LimitError::TooLarge) => Err(LimitError::TooLarge)?,
                Err(_) => {
                    item = Some(it);
                    run_now = true;
                }
                Ok(_) => {
                    pending = true;
                }
            }
        } else if pending {
            run_now = true;
        }

        if pending && run_now {
            if vflag {
                let mut args = String::new();
                for arg in cmd.get_args() {
                    args.push(' ');
                    args.push_str(&arg.to_string_lossy());
                }
                eprintln!("{}{}", cmd.get_program().to_string_lossy(), args);
            }
            let res = cmd.into_command().status()?;
            if !res.success() {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    if let Some(signal) = res.signal() {
                        eprintln!(
                            "{}: terminated with signal {}; aborting",
                            cmd.get_program().to_string_lossy(),
                            signal
                        );
                        return Ok(0);
                    }
                }
                rc = res.code().unwrap_or(1);
                if rc == 255 {
                    eprintln!(
                        "{}: exited with status 255; aborting",
                        cmd.get_program().to_string_lossy()
                    );
                    return Ok(rc);
                }
            }
            pending = false;
            run_now = false;
            cmd = basecmd.clone();
        }
    }

    Ok(rc)
}
