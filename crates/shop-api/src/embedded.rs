use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
use std::time::Duration;

use postgresql_embedded::{PostgreSQL, SettingsBuilder, VersionReq};

const DATABASE: &str = "shop";
/// Same string as `POSTGRESQL_VERSION` in `.cargo/config.toml`.
/// PostgreSQL 18's initdb dies on the shop Windows PC during post-bootstrap
/// (`0xC0000005`), including with checksums turned off. 16 matches the shop
/// database and gets through that step.
const POSTGRES_VERSION: &str = "=16.16.0";
#[cfg(target_os = "windows")]
static PG_LAUNCHER: OnceLock<&'static [u8]> = OnceLock::new();

pub(crate) fn set_pg_launcher(bytes: &'static [u8]) {
    #[cfg(target_os = "windows")]
    {
        let _ = PG_LAUNCHER.set(bytes);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = bytes;
}

pub struct EmbeddedDb {
    server: PostgreSQL,
    pub url: String,
    _lock: fs::File,
}

impl EmbeddedDb {
    pub async fn start() -> Result<Self, String> {
        let home = till_home()?;
        let lock = lock_till(&home)?;
        fs::create_dir_all(home.join("item-images")).map_err(|err| err.to_string())?;
        // Picture files for this database stay beside it, away from any other shop data.
        std::env::set_var("IMAGE_DIR", home.join("item-images"));

        let password = load_or_create_password(&home.join("db.password"))?;
        let port = pg_port()?;
        // A previous launch can be killed while initdb is still creating files.
        // Anything without pg_control is not a database yet and cannot be started.
        discard_incomplete_cluster(&home)?;
        // The bundled initdb forces UTF-8 and the Windows ANSI locale, which
        // refuse each other. Plant a conf file so setup only unpacks the
        // server, then initialize with --no-locale ourselves.
        #[cfg(target_os = "windows")]
        let init_on_windows = !cluster_is_complete(&home.join("pgdata"));
        #[cfg(target_os = "windows")]
        if init_on_windows {
            fs::create_dir_all(home.join("pgdata")).map_err(|err| err.to_string())?;
            fs::write(home.join("pgdata").join("postgresql.conf"), b"")
                .map_err(|err| err.to_string())?;
        }

        let mut server = database_server(&home, &password, port);
        if let Err(err) = server.setup().await {
            let message = with_database_log(&home, err.to_string());
            #[cfg(target_os = "windows")]
            if init_on_windows {
                let _ = fs::remove_dir_all(home.join("pgdata"));
            }
            return Err(message);
        }
        // Stop a database left open before replacing postgres.exe. Windows will
        // not let us rewrite a program that is still running.
        #[cfg(target_os = "windows")]
        let install_dir = server.settings().installation_dir.clone();
        stop_leftover(&home)?;
        #[cfg(target_os = "windows")]
        install_postgres_launcher(&install_dir)?;
        #[cfg(target_os = "windows")]
        if init_on_windows {
            if !cluster_is_complete(&home.join("pgdata")) {
                fs::remove_dir_all(home.join("pgdata"))
                    .map_err(|err| format!("Could not reset the shop database: {err}"))?;
            }
            windows_initdb(&install_dir, &home, &password).await?;
        }
        server
            .start()
            .await
            .map_err(|err| with_database_log(&home, err.to_string()))?;
        if !server
            .database_exists(DATABASE)
            .await
            .map_err(|err| err.to_string())?
        {
            server
                .create_database(DATABASE)
                .await
                .map_err(|err| err.to_string())?;
        }
        let url = server.settings().url(DATABASE);
        tracing::info!(
            "Shop database ready on 127.0.0.1:{port}, data in {}",
            home.join("pgdata").display()
        );
        Ok(Self {
            server,
            url,
            _lock: lock,
        })
    }

    pub async fn stop(self) {
        if let Err(err) = self.server.stop().await {
            tracing::error!("Could not stop the shop database: {err}");
        }
    }
}

fn till_home() -> Result<PathBuf, String> {
    let home = if let Ok(dir) = std::env::var("TILL_HOME") {
        if !dir.trim().is_empty() {
            PathBuf::from(dir)
        } else {
            default_home()
        }
    } else {
        default_home()
    };
    fs::create_dir_all(&home).map_err(|err| format!("{}: {err}", home.display()))?;
    Ok(home)
}

fn default_home() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("config.toml").is_file() {
            return cwd.join("data/till");
        }
    }
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir).join("shop-till");
        }
    }
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir).join("shop-till");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/shop-till");
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if !profile.trim().is_empty() {
            return PathBuf::from(profile).join("AppData/Local/shop-till");
        }
    }
    PathBuf::from("data/till")
}

fn lock_till(home: &Path) -> Result<fs::File, String> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(home.join("till.lock"))
        .map_err(|err| err.to_string())?;
    file.try_lock().map_err(|err| match err {
        std::fs::TryLockError::WouldBlock => "The till is already open.".to_string(),
        std::fs::TryLockError::Error(err) => err.to_string(),
    })?;
    let _ = file.write_all(std::process::id().to_string().as_bytes());
    Ok(file)
}

fn database_server(home: &Path, password: &str, port: u16) -> PostgreSQL {
    let mut configuration = HashMap::new();
    configuration.insert("listen_addresses".into(), "127.0.0.1".into());
    // The crate default kills initdb after 5 seconds. Unpacking and creating
    // the cluster on a shop PC takes longer than that.
    let settings = SettingsBuilder::new()
        .installation_dir(home.join("postgres"))
        .password_file(home.join("pgpass"))
        .data_dir(home.join("pgdata"))
        .host("127.0.0.1")
        .port(port)
        .username("postgres")
        .password(password)
        .temporary(false)
        .timeout(Some(Duration::from_secs(180)))
        .version(VersionReq::parse(POSTGRES_VERSION).expect("postgres version"))
        .configuration(configuration)
        .build();
    PostgreSQL::new(settings)
}

fn cluster_is_complete(data: &Path) -> bool {
    data.join("global").join("pg_control").is_file()
}

fn discard_incomplete_cluster(home: &Path) -> Result<(), String> {
    let data = home.join("pgdata");
    if !data.exists() || cluster_is_complete(&data) {
        return Ok(());
    }
    let _ = stop_leftover(home);
    fs::remove_dir_all(&data).map_err(|err| format!("Could not reset the shop database: {err}"))
}

fn with_database_log(home: &Path, mut message: String) -> String {
    for name in ["start.log", "initdb.log"] {
        let path = home.join("pgdata").join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let tail = text.chars().rev().take(1200).collect::<String>();
        let tail = tail.chars().rev().collect::<String>();
        message.push_str("\n\n");
        message.push_str(&path.display().to_string());
        message.push_str(":\n");
        message.push_str(&tail);
    }
    if message.contains("deadline has elapsed") {
        message = format!("Setting up the shop database took too long.\n\n{message}");
    }
    message
}

#[cfg(target_os = "windows")]
fn install_postgres_launcher(install_dir: &Path) -> Result<(), String> {
    let bytes = PG_LAUNCHER.get().copied().unwrap_or(&[]);
    if bytes.is_empty() {
        return Err("This build cannot start the shop database without a terminal.".to_string());
    }
    let bin = find_bin(install_dir, &["pg_ctl.exe", "pg_ctl"])
        .and_then(|program| program.parent().map(Path::to_path_buf))
        .ok_or_else(|| "Could not find the shop database programs.".to_string())?;
    let postgres = bin.join("postgres.exe");
    let server = bin.join("postgres-server.exe");
    if !server.exists() {
        if !postgres.exists() {
            return Err("Could not find postgres.exe.".to_string());
        }
        fs::rename(&postgres, &server).map_err(|err| format!("{}: {err}", postgres.display()))?;
    }
    fs::write(&postgres, bytes).map_err(|err| format!("{}: {err}", postgres.display()))
}

#[cfg(target_os = "windows")]
async fn windows_initdb(install_dir: &Path, home: &Path, password: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let data = home.join("pgdata");
    fs::create_dir_all(&data).map_err(|err| err.to_string())?;
    let pwfile = home.join("pgpass");
    fs::write(&pwfile, password).map_err(|err| format!("{}: {err}", pwfile.display()))?;
    let initdb = find_bin(install_dir, &["initdb.exe", "initdb"])
        .ok_or_else(|| "Could not find initdb.exe in the shop database install.".to_string())?;
    let log_path = home.join("initdb.log");
    let log_file =
        fs::File::create(&log_path).map_err(|err| format!("{}: {err}", log_path.display()))?;
    let log_err = log_file
        .try_clone()
        .map_err(|err| format!("{}: {err}", log_path.display()))?;
    // CREATE_NO_WINDOW. initdb is only the setup step; the launcher keeps the
    // server itself from opening a terminal too.
    let mut command = tokio::process::Command::new(&initdb);
    command.creation_flags(0x0800_0000);
    if let Some(bin_dir) = initdb.parent() {
        command.current_dir(bin_dir);
    }
    command
        .arg("--pgdata")
        .arg(&data)
        .arg("--username")
        .arg("postgres")
        .arg("--auth")
        .arg("password")
        .arg("--pwfile")
        .arg(&pwfile)
        .arg("--encoding")
        .arg("UTF8")
        .arg("--no-locale")
        .arg("--no-data-checksums")
        .arg("--no-instructions")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_err));
    let status = tokio::time::timeout(Duration::from_secs(180), command.status())
        .await
        .map_err(|_| "Setting up the shop database took too long.".to_string())?
        .map_err(|err| format!("Could not set up the shop database: {err}"))?;
    if status.success() {
        return Ok(());
    }
    let log = fs::read_to_string(&log_path).unwrap_or_default();
    Err(format!(
        "Could not set up the shop database.\n{}",
        log.trim()
    ))
}

fn stop_leftover(home: &Path) -> Result<(), String> {
    let data = home.join("pgdata");
    let pid = data.join("postmaster.pid");
    if !pid.exists() {
        return Ok(());
    }
    let Some(pg_ctl) = find_pg_ctl(&home.join("postgres")) else {
        return Ok(());
    };
    let running = database_command(&pg_ctl)
        .arg("-D")
        .arg(&data)
        .arg("status")
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !running {
        let _ = fs::remove_file(pid);
        return Ok(());
    }
    tracing::info!("Stopping a shop database left open by the last run");
    let stopped = database_command(&pg_ctl)
        .arg("-D")
        .arg(&data)
        .args(["-m", "fast", "stop"])
        .status()
        .map_err(|err| err.to_string())?;
    if stopped.success() {
        Ok(())
    } else {
        Err("Could not stop the shop database left open by the last run.".into())
    }
}

fn find_pg_ctl(installation: &Path) -> Option<PathBuf> {
    find_bin(installation, &["pg_ctl", "pg_ctl.exe"])
}

fn find_bin(installation: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        let direct = installation.join("bin").join(name);
        if direct.is_file() {
            return Some(direct);
        }
    }
    let entries = fs::read_dir(installation).ok()?;
    for entry in entries.flatten() {
        for name in names {
            let candidate = entry.path().join("bin").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn database_command(program: &Path) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new(program);
        command.creation_flags(0x0800_0000);
        command
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(program)
    }
}

fn pg_port() -> Result<u16, String> {
    match std::env::var("TILL_PG_PORT") {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("TILL_PG_PORT must be a port number, got {value}")),
        Err(_) => Ok(5434),
    }
}

fn load_or_create_password(path: &Path) -> Result<String, String> {
    if let Ok(existing) = fs::read_to_string(path) {
        let password = existing.trim().to_string();
        if !password.is_empty() {
            return Ok(password);
        }
    }
    let password = random_password()?;
    fs::write(path, &password).map_err(|err| format!("{}: {err}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interrupted_cluster_has_no_control_file() {
        let dir = std::env::temp_dir().join(format!("shop-till-cluster-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(!cluster_is_complete(&dir));
        fs::create_dir_all(dir.join("global")).unwrap();
        fs::write(dir.join("postgresql.conf"), b"").unwrap();
        assert!(!cluster_is_complete(&dir));
        fs::write(dir.join("global").join("pg_control"), b"").unwrap();
        assert!(cluster_is_complete(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
}

fn random_password() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).map_err(|err| err.to_string())?;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    Ok(bytes
        .iter()
        .map(|byte| ALPHABET[(*byte as usize) % ALPHABET.len()] as char)
        .collect())
}
