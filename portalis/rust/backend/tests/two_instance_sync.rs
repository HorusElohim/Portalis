use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const FIXTURE: &str = "portalis two-instance fixture v1\n";

struct PeerProcess {
    child: Child,
    lines: Receiver<String>,
}

impl Drop for PeerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn two_instances_sync_and_auto_fetch_a_shared_fixture() {
    let root =
        std::env::temp_dir().join(format!("portalis-two-instance-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let fixture = root.join("fixture.txt");
    std::fs::write(&fixture, FIXTURE).unwrap();

    let owner_port = free_port();
    let joiner_port = loop {
        let port = free_port();
        if port != owner_port {
            break port;
        }
    };
    let owner_download_dir = root.join("owner-downloads");
    let owner_download_dir = owner_download_dir.to_string_lossy().into_owned();
    let owner = spawn_peer(
        &root,
        "owner",
        [
            fixture.to_string_lossy().as_ref(),
            owner_download_dir.as_str(),
            &owner_port.to_string(),
        ],
    );
    let owner_ready = wait_for(&owner.lines, "OWNER_READY ");
    let owner_fields: Vec<_> = owner_ready.split_whitespace().collect();
    assert_eq!(owner_fields.len(), 4, "owner output: {owner_ready}");
    let invite = owner_fields[1];
    let owner_info_hash = owner_fields[3];

    let joiner_download_dir = root.join("joiner-downloads");
    let joiner_download_dir = joiner_download_dir.to_string_lossy().into_owned();
    let joiner = spawn_peer(
        &root,
        "joiner",
        [
            invite,
            FIXTURE,
            joiner_download_dir.as_str(),
            &joiner_port.to_string(),
        ],
    );
    let joined = wait_for(&joiner.lines, "JOINED ");
    assert!(
        !joined["JOINED ".len()..].is_empty(),
        "joiner did not return an id"
    );

    let fetched = wait_for(&joiner.lines, "FETCHED ");
    let fetched_fields: Vec<_> = fetched.split_whitespace().collect();
    assert_eq!(fetched_fields.len(), 3, "joiner output: {fetched}");
    assert_eq!(fetched_fields[1], owner_info_hash);
    assert!(std::path::Path::new(fetched_fields[2]).is_file());

    drop(joiner);
    drop(owner);
    let _ = std::fs::remove_dir_all(root);
}

fn spawn_peer<const N: usize>(root: &std::path::Path, mode: &str, args: [&str; N]) -> PeerProcess {
    let home = root.join(mode);
    std::fs::create_dir_all(&home).unwrap();
    let peer_binary = std::env::var_os("CARGO_BIN_EXE_portalis_peer")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()?
                .parent()
                .map(|target_debug| target_debug.join("portalis-peer"))
        })
        .filter(|path| path.is_file())
        .expect("could not locate the portalis-peer test helper");
    let mut command = Command::new(peer_binary);
    command
        .arg(mode)
        .args(args)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("RUST_BACKTRACE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let lines = spawn_line_reader(stdout);
    PeerProcess { child, lines }
}

fn spawn_line_reader(stdout: ChildStdout) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

fn wait_for(lines: &Receiver<String>, prefix: &str) -> String {
    let deadline = std::time::Instant::now() + Duration::from_secs(75);
    let mut observed = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let line = lines.recv_timeout(remaining).unwrap_or_else(|error| {
            panic!("waiting for {prefix:?}; observed {observed:?}: {error}")
        });
        if line.starts_with(prefix) {
            return line;
        }
        observed.push(line);
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.local_addr().unwrap().port()
}
