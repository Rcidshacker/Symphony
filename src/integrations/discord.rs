//! Discord Rich Presence over Discord's local IPC.
//!
//! The desktop client listens on a named pipe (Windows) or unix socket (macOS, Linux). After a
//! handshake carrying your application's client id, `SET_ACTIVITY` frames show what is playing.
//! Create a Discord application at <https://discord.com/developers/applications> and put its
//! "Application ID" in `[plugins.discord] client_id`.

use std::io::{self, Read, Write};

use serde_json::{json, Value};

/// Opcodes of the IPC framing
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;

/// A frame bigger than this is not a Discord response
const MAX_FRAME_BYTES: u32 = 1 << 20;

/// What Discord shows under your name
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Activity {
    /// First line (the track title)
    pub details: String,
    /// Second line (the artist, or "Paused")
    pub state: String,
    /// Unix time the track started, shown as elapsed time (`None` hides the timer)
    pub start_unix: Option<i64>,
}

/// A byte stream to the Discord client
pub trait Ipc: Read + Write + Send {}
impl<T: Read + Write + Send> Ipc for T {}

type Connector = Box<dyn Fn() -> io::Result<Box<dyn Ipc>> + Send + Sync>;

/// Connection to the local Discord client; connects on first use and reconnects once if it drops
pub struct DiscordPresence {
    client_id: String,
    connect: Connector,
    conn: Option<Box<dyn Ipc>>,
    nonce: u64,
}

impl DiscordPresence {
    /// Presence for the Discord application `client_id`, talking to the real client
    pub fn new(client_id: impl Into<String>) -> Self {
        Self::with_connector(client_id, Box::new(connect_local))
    }

    /// Presence over a custom transport (used by tests)
    pub fn with_connector(client_id: impl Into<String>, connect: Connector) -> Self {
        Self {
            client_id: client_id.into(),
            connect,
            conn: None,
            nonce: 0,
        }
    }

    /// Show `activity`
    pub fn set_activity(&mut self, activity: &Activity) -> io::Result<()> {
        let mut args = json!({ "pid": std::process::id(), "activity": {
            "details": activity.details,
            "state": activity.state,
        }});
        if let Some(start) = activity.start_unix {
            args["activity"]["timestamps"] = json!({ "start": start });
        }
        self.command(args)
    }

    /// Remove the presence
    pub fn clear(&mut self) -> io::Result<()> {
        self.command(json!({ "pid": std::process::id(), "activity": null }))
    }

    /// Send `SET_ACTIVITY` with `args`; after a broken connection, reconnect and try once more
    fn command(&mut self, args: Value) -> io::Result<()> {
        match self.try_command(&args) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.conn = None;
                self.try_command(&args).inspect_err(|_| self.conn = None)
            }
        }
    }

    fn try_command(&mut self, args: &Value) -> io::Result<()> {
        if self.conn.is_none() {
            self.conn = Some(self.handshake()?);
        }
        self.nonce += 1;
        let payload =
            json!({ "cmd": "SET_ACTIVITY", "args": args, "nonce": self.nonce.to_string() });
        let conn = self.conn.as_mut().ok_or(io::ErrorKind::NotConnected)?;
        write_frame(conn, OP_FRAME, &payload)?;
        // Discord answers every command; an error response means the activity was refused
        let (_, response) = read_frame(conn)?;
        match response["evt"].as_str() {
            Some("ERROR") => Err(io::Error::other(format!(
                "Discord refused the activity: {}",
                response["data"]["message"]
                    .as_str()
                    .unwrap_or("unknown error")
            ))),
            _ => Ok(()),
        }
    }

    fn handshake(&self) -> io::Result<Box<dyn Ipc>> {
        let mut conn = (self.connect)()?;
        write_frame(
            &mut conn,
            OP_HANDSHAKE,
            &json!({ "v": 1, "client_id": self.client_id }),
        )?;
        let (_, ready) = read_frame(&mut conn)?;
        if ready["evt"].as_str() != Some("READY") {
            return Err(io::Error::other(format!(
                "Discord did not accept the client id: {ready}"
            )));
        }
        Ok(conn)
    }
}

/// `opcode (u32 LE) | length (u32 LE) | JSON`
pub fn write_frame(writer: &mut impl Write, opcode: u32, payload: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(payload)?;
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    writer.write_all(&frame)?;
    writer.flush()
}

/// Read one frame
pub fn read_frame(reader: &mut impl Read) -> io::Result<(u32, Value)> {
    let mut header = [0u8; 8];
    reader.read_exact(&mut header)?;
    let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized Discord frame",
        ));
    }
    let mut body = vec![0u8; len as usize];
    reader.read_exact(&mut body)?;
    Ok((opcode, serde_json::from_slice(&body)?))
}

/// Open Discord's IPC endpoint (`discord-ipc-0` .. `-9`)
#[cfg(windows)]
fn connect_local() -> io::Result<Box<dyn Ipc>> {
    for n in 0..10 {
        let pipe = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!(r"\\.\pipe\discord-ipc-{n}"));
        if let Ok(pipe) = pipe {
            return Ok(Box::new(pipe));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Discord is not running",
    ))
}

#[cfg(unix)]
fn connect_local() -> io::Result<Box<dyn Ipc>> {
    let dirs = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .chain(["/tmp".to_string()]);
    for dir in dirs {
        for n in 0..10 {
            if let Ok(socket) =
                std::os::unix::net::UnixStream::connect(format!("{dir}/discord-ipc-{n}"))
            {
                return Ok(Box::new(socket));
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Discord is not running",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    /// Discord stand-in: replies `replies` in order, records what it was sent
    struct Fake {
        sent: Arc<Mutex<Vec<u8>>>,
        replies: Cursor<Vec<u8>>,
        break_writes: bool,
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.replies.read(buf)
        }
    }

    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.break_writes {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.sent.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn frames(replies: &[Value]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for reply in replies {
            write_frame(&mut bytes, OP_FRAME, reply).unwrap();
        }
        bytes
    }

    fn parse(sent: &[u8]) -> Vec<(u32, Value)> {
        let mut cursor = Cursor::new(sent);
        let mut out = Vec::new();
        while (cursor.position() as usize) < sent.len() {
            out.push(read_frame(&mut cursor).unwrap());
        }
        out
    }

    #[test]
    fn handshake_then_activity_frames() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let transport = sent.clone();
        let mut presence = DiscordPresence::with_connector(
            "1234",
            Box::new(move || {
                Ok(Box::new(Fake {
                    sent: transport.clone(),
                    replies: Cursor::new(frames(&[
                        json!({"cmd":"DISPATCH","evt":"READY"}),
                        json!({"cmd":"SET_ACTIVITY","evt":null}),
                        json!({"cmd":"SET_ACTIVITY","evt":null}),
                    ])),
                    break_writes: false,
                }) as Box<dyn Ipc>)
            }),
        );

        presence
            .set_activity(&Activity {
                details: "Song".into(),
                state: "Artist".into(),
                start_unix: Some(1_700_000_000),
            })
            .unwrap();
        presence.clear().unwrap();

        let frames = parse(&sent.lock().unwrap());
        assert_eq!(frames.len(), 3, "one handshake, then one frame per command");
        assert_eq!(
            frames[0],
            (OP_HANDSHAKE, json!({"v": 1, "client_id": "1234"}))
        );
        let activity = &frames[1].1;
        assert_eq!(frames[1].0, OP_FRAME);
        assert_eq!(activity["cmd"], "SET_ACTIVITY");
        assert_eq!(activity["args"]["activity"]["details"], "Song");
        assert_eq!(activity["args"]["activity"]["state"], "Artist");
        assert_eq!(
            activity["args"]["activity"]["timestamps"]["start"],
            1_700_000_000
        );
        assert_eq!(activity["args"]["pid"], std::process::id());
        assert!(
            frames[2].1["args"]["activity"].is_null(),
            "clear sends a null activity"
        );
        assert_ne!(activity["nonce"], frames[2].1["nonce"]);
    }

    #[test]
    fn a_dropped_connection_is_reopened_once() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let (transport, calls) = (sent.clone(), Arc::new(Mutex::new(0)));
        let counter = calls.clone();
        let mut presence = DiscordPresence::with_connector(
            "1234",
            Box::new(move || {
                let mut n = counter.lock().unwrap();
                *n += 1;
                // the first connection dies on the first command, the second one works
                Ok(Box::new(Fake {
                    sent: transport.clone(),
                    replies: Cursor::new(frames(&[
                        json!({"cmd":"DISPATCH","evt":"READY"}),
                        json!({"cmd":"SET_ACTIVITY","evt":null}),
                    ])),
                    break_writes: *n == 1,
                }) as Box<dyn Ipc>)
            }),
        );
        // the broken first connection fails during its handshake write, then the retry succeeds
        presence.set_activity(&Activity::default()).unwrap();
        assert_eq!(*calls.lock().unwrap(), 2);
    }

    #[test]
    fn errors_surface_when_discord_is_missing_or_refuses() {
        let mut absent = DiscordPresence::with_connector(
            "1",
            Box::new(|| {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Discord is not running",
                ))
            }),
        );
        assert!(absent.set_activity(&Activity::default()).is_err());

        let mut refused = DiscordPresence::with_connector(
            "bad",
            Box::new(|| {
                Ok(Box::new(Fake {
                    sent: Arc::default(),
                    replies: Cursor::new(frames(&[
                        json!({"cmd":"DISPATCH","evt":"ERROR","data":{"message":"Invalid Client ID"}}),
                    ])),
                    break_writes: false,
                }) as Box<dyn Ipc>)
            }),
        );
        let error = refused
            .set_activity(&Activity::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("client id"), "{error}");
    }

    #[test]
    fn oversized_frames_are_rejected() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_frame(&mut Cursor::new(bytes)).is_err());
    }
}
