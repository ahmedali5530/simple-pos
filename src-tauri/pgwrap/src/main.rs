//! Stands in for `cmd.exe` when PostgreSQL starts the database.
//!
//! `pg_ctl` launches the server as
//! `COMSPEC /D /C "postgres ... < nul >> start.log 2>&1"`.
//! `cmd.exe` is a console program, so Windows opens a terminal for it and
//! leaves that terminal open until the database stops. This program is a
//! windowed replacement: it runs that one command with no window and waits.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    #[cfg(target_os = "windows")]
    {
        let Some(line) = command_line() else {
            return 1;
        };
        let Some(script) = command_script(&line) else {
            return 1;
        };
        return launch(&script);
    }
    #[cfg(not(target_os = "windows"))]
    {
        1
    }
}

/// The command `pg_ctl` places after `/C`.
fn command_script(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let marker = lower.find(" /c ")?;
    let mut rest = line[marker + 4..].trim();
    if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
        rest = &rest[1..rest.len() - 1];
    }
    let script = rest.trim();
    if script.is_empty() {
        None
    } else {
        Some(script.to_string())
    }
}

struct Launch {
    program: String,
    args: Vec<String>,
    stdin: Option<String>,
    stdout: Option<String>,
    append: bool,
    stderr_to_stdout: bool,
}

fn parse_launch(script: &str) -> Option<Launch> {
    let tokens = tokenize(script)?;
    let mut program = None;
    let mut args = Vec::new();
    let mut stdin = None;
    let mut stdout = None;
    let mut append = false;
    let mut stderr_to_stdout = false;
    let mut index = 0;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "<" => {
                stdin = Some(tokens.get(index + 1)?.clone());
                index += 2;
            }
            ">" => {
                stdout = Some(tokens.get(index + 1)?.clone());
                append = false;
                index += 2;
            }
            ">>" => {
                stdout = Some(tokens.get(index + 1)?.clone());
                append = true;
                index += 2;
            }
            "2>&1" => {
                stderr_to_stdout = true;
                index += 1;
            }
            _ => {
                if program.is_none() {
                    program = Some(tokens[index].clone());
                } else {
                    args.push(tokens[index].clone());
                }
                index += 1;
            }
        }
    }
    Some(Launch {
        program: program?,
        args,
        stdin,
        stdout,
        append,
        stderr_to_stdout,
    })
}

fn tokenize(input: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in input.chars() {
        match character {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if quoted {
        return None;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Some(tokens)
}

#[cfg(target_os = "windows")]
fn command_line() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    unsafe extern "system" {
        fn GetCommandLineW() -> *const u16;
    }
    unsafe {
        let pointer = GetCommandLineW();
        if pointer.is_null() {
            return None;
        }
        let mut length = 0;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        let slice = std::slice::from_raw_parts(pointer, length);
        Some(OsString::from_wide(slice).to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "windows")]
fn launch(script: &str) -> i32 {
    let Some(plan) = parse_launch(script) else {
        return 1;
    };
    match spawn(&plan) {
        Ok(code) => code,
        Err(err) => {
            if let Some(log) = &plan.stdout {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log)
                    .and_then(|mut file| {
                        use std::io::Write;
                        writeln!(file, "Could not start the database: {err}")
                    });
            }
            1
        }
    }
}

#[cfg(target_os = "windows")]
fn spawn(plan: &Launch) -> Result<i32, String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let mut command = Command::new(&plan.program);
    command.args(&plan.args);
    // The shop window has no console. Without this, Windows opens one for
    // postgres and leaves it open for the whole session.
    command.creation_flags(0x0800_0000);
    if let Some(path) = &plan.stdin {
        let file = std::fs::File::open(path).map_err(|err| format!("{path}: {err}"))?;
        command.stdin(Stdio::from(file));
    } else {
        command.stdin(Stdio::null());
    }
    if let Some(path) = &plan.stdout {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).write(true);
        if plan.append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let file = options
            .open(path)
            .map_err(|err| format!("{path}: {err}"))?;
        if plan.stderr_to_stdout {
            let errors = file.try_clone().map_err(|err| err.to_string())?;
            command.stderr(Stdio::from(errors));
        } else {
            command.stderr(Stdio::null());
        }
        command.stdout(Stdio::from(file));
    }
    let status = command.status().map_err(|err| err.to_string())?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::{command_script, parse_launch};

    #[test]
    fn reads_the_postgres_16_start_line() {
        let line = r#""C:\Users\ahmed\AppData\Local\shop-till\shop-cmd.exe" /D /C ""C:\Users\ahmed\AppData\Local\shop-till\postgres\16.16.0\bin\postgres.exe" -D "C:\Users\ahmed\AppData\Local\shop-till\pgdata" -F -p 5434 -c listen_addresses=127.0.0.1 < "nul" >> "C:\Users\ahmed\AppData\Local\shop-till\pgdata\start.log" 2>&1""#;
        let script = command_script(line).unwrap();
        let launch = parse_launch(&script).unwrap();
        assert_eq!(
            launch.program,
            r"C:\Users\ahmed\AppData\Local\shop-till\postgres\16.16.0\bin\postgres.exe"
        );
        assert_eq!(
            launch.args,
            [
                "-D",
                r"C:\Users\ahmed\AppData\Local\shop-till\pgdata",
                "-F",
                "-p",
                "5434",
                "-c",
                "listen_addresses=127.0.0.1",
            ]
        );
        assert_eq!(launch.stdin.as_deref(), Some("nul"));
        assert_eq!(
            launch.stdout.as_deref(),
            Some(r"C:\Users\ahmed\AppData\Local\shop-till\pgdata\start.log")
        );
        assert!(launch.append);
        assert!(launch.stderr_to_stdout);
    }
}
