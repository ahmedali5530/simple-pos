use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use postgresql_embedded::{PostgreSQL, SettingsBuilder};

const DATABASE: &str = "shop";

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
        let mut configuration = HashMap::new();
        configuration.insert("listen_addresses".into(), "127.0.0.1".into());
        let settings = SettingsBuilder::new()
            .installation_dir(home.join("postgres"))
            .password_file(home.join("pgpass"))
            .data_dir(home.join("pgdata"))
            .host("127.0.0.1")
            .port(port)
            .username("postgres")
            .password(password)
            .temporary(false)
            .configuration(configuration)
            .build();

        let mut server = PostgreSQL::new(settings);
        server.setup().await.map_err(|err| err.to_string())?;
        stop_leftover(&home)?;
        server.start().await.map_err(|err| err.to_string())?;
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

fn stop_leftover(home: &Path) -> Result<(), String> {
    let data = home.join("pgdata");
    let pid = data.join("postmaster.pid");
    if !pid.exists() {
        return Ok(());
    }
    let Some(pg_ctl) = find_pg_ctl(&home.join("postgres")) else {
        return Ok(());
    };
    let running = std::process::Command::new(&pg_ctl)
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
    let stopped = std::process::Command::new(pg_ctl)
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
    for name in ["pg_ctl", "pg_ctl.exe"] {
        let direct = installation.join("bin").join(name);
        if direct.is_file() {
            return Some(direct);
        }
    }
    let entries = fs::read_dir(installation).ok()?;
    for entry in entries.flatten() {
        for name in ["pg_ctl", "pg_ctl.exe"] {
            let candidate = entry.path().join("bin").join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
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

fn random_password() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).map_err(|err| err.to_string())?;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    Ok(bytes
        .iter()
        .map(|byte| ALPHABET[(*byte as usize) % ALPHABET.len()] as char)
        .collect())
}
