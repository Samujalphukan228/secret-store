use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{PasswordHash, PasswordVerifier, SaltString};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use chacha20poly1305::aead::{Aead, Payload};
use clap::{Parser, Subcommand};
use colored::*;
use dirs::home_dir;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "secret", about = "CLI secret manager for developers", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias = "ss")]
    Set { key: String, value: String },
    #[command(alias = "sg")]
    Get { key: String },
    #[command(alias = "sc")]
    Copy { key: String },
    #[command(alias = "sl")]
    List,
    #[command(alias = "sd")]
    Delete { key: String },
    Init,
    #[command(alias = "cp")]
    ChangePassword,
}

#[derive(Serialize, Deserialize, Clone)]
struct Secret {
    key: String,
    encrypted_value: String,
    created_at: String,
}

#[derive(Serialize, Deserialize)]
struct SecretStore {
    password_hash: String,
    salt: String,
    secrets: Vec<Secret>,
}

fn get_store_dir() -> PathBuf {
    let mut path = home_dir().expect("Could not find home directory");
    path.push(".secret-store");
    path
}

fn get_store_path() -> PathBuf {
    let mut path = get_store_dir();
    path.push("secrets.json");
    path
}

fn load_store() -> Option<SecretStore> {
    let path = get_store_path();
    if !path.exists() {
        return None;
    }
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_store(store: &SecretStore) {
    let dir = get_store_dir();
    let path = get_store_path();

    if !dir.exists() {
        fs::create_dir_all(&dir).expect("Failed to create .secret-store directory");
    }

    let json = serde_json::to_string_pretty(store).expect("Failed to serialize secrets");
    fs::write(&path, json).expect("Failed to write secrets file");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("Failed to set file permissions");
    }
}

fn hash_password(password: &str, salt: &SaltString) -> String {
    Argon2::default()
        .hash_password(password.as_bytes(), salt)
        .unwrap()
        .to_string()
}

fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = PasswordHash::new(hash).unwrap();
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn derive_key(password: &str) -> Key {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    password.hash(&mut hasher);
    let hash = hasher.finish();

    let mut key_bytes = [0u8; 32];
    for i in 0..8 {
        key_bytes[i] = ((hash >> (i * 8)) & 0xff) as u8;
        key_bytes[i + 8] = ((hash >> ((i + 8) * 8)) & 0xff) as u8;
    }
    for i in 16..32 {
        key_bytes[i] = key_bytes[i - 16] ^ key_bytes[i - 8];
    }

    Key::from(key_bytes)
}

fn encrypt_secret(value: &str, password: &str) -> String {
    let key_bytes = derive_key(password);
    let cipher = ChaCha20Poly1305::new(&key_bytes);

    let nonce_bytes = rand::thread_rng().gen::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, Payload::from(value.as_bytes()))
        .expect("Encryption failed");

    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);
    hex::encode(combined)
}

fn decrypt_secret(encrypted: &str, password: &str) -> Result<String, String> {
    let key_bytes = derive_key(password);
    let cipher = ChaCha20Poly1305::new(&key_bytes);

    let combined = hex::decode(encrypted).map_err(|_| "Invalid encrypted data")?;

    if combined.len() < 12 {
        return Err("Invalid encrypted data".to_string());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, Payload::from(ciphertext))
        .map_err(|_| "Decryption failed - wrong password?")?;

    String::from_utf8(plaintext).map_err(|_| "Invalid UTF-8".to_string())
}

fn prompt_password(prompt: &str) -> String {
    eprint!("{}", prompt.cyan().bold());
    io::stderr().flush().ok();
    rpassword::read_password().expect("Failed to read password")
}

fn ensure_authenticated(store: &SecretStore) -> String {
    let password = prompt_password("Enter master password: ");
    if !verify_password(&password, &store.password_hash) {
        eprintln!("{} Wrong password", "✗".red().bold());
        std::process::exit(1);
    }
    password
}

fn matches_fuzzy(name_lower: &str, query: &str) -> bool {
    let query_lower = query.to_lowercase();

    let initials: String = name_lower
        .split(|c: char| c == '-' || c == '_')
        .filter_map(|seg| seg.chars().next())
        .collect();

    if initials.contains(&query_lower) {
        return true;
    }

    let mut qchars = query_lower.chars().peekable();
    for ch in name_lower.chars() {
        if qchars.peek().map(|q| *q == ch).unwrap_or(false) {
            qchars.next();
        }
    }
    qchars.peek().is_none()
}

fn find_secret_fuzzy(store: &SecretStore, query: &str) -> Option<Secret> {
    let query_lower = query.to_lowercase();

    let matches: Vec<_> = store
        .secrets
        .iter()
        .filter(|s| {
            let name_lower = s.key.to_lowercase();
            name_lower.contains(&query_lower) || matches_fuzzy(&name_lower, &query)
        })
        .cloned()
        .collect();

    match matches.len() {
        0 => None,
        1 => Some(matches[0].clone()),
        _ => {
            println!("{} Multiple matches for '{}':\n", "→".cyan().bold(), query.yellow());

            for (i, secret) in matches.iter().enumerate() {
                println!(
                    "  {}  {}",
                    format!("{}", i + 1).yellow().bold(),
                    secret.key.bright_cyan()
                );
            }

            println!();
            eprint!("{}", "Pick a number: ".cyan().bold());
            io::stderr().flush().ok();

            let stdin = io::stdin();
            let line = stdin.lock().lines().next().unwrap().unwrap_or_default();
            let index: usize = line.trim().parse().unwrap_or(1);
            let index = index.saturating_sub(1).min(matches.len() - 1);

            Some(matches[index].clone())
        }
    }
}

fn init_store() {
    let dir = get_store_dir();
    let path = get_store_path();

    if dir.exists() && path.exists() {
        println!("{} Secret store already initialized", "→".cyan().bold());
        return;
    }

    let password = prompt_password("Enter master password: ");
    let password_confirm = prompt_password("Confirm password: ");

    if password != password_confirm {
        eprintln!("{} Passwords don't match", "✗".red().bold());
        std::process::exit(1);
    }

    fs::create_dir_all(&dir).expect("Failed to create .secret-store directory");

    let salt = SaltString::generate(rand::thread_rng());
    let password_hash = hash_password(&password, &salt);

    let store = SecretStore {
        password_hash,
        salt: salt.to_string(),
        secrets: vec![],
    };

    save_store(&store);

    println!(
        "{} Secret store initialized at {}",
        "✓".green().bold(),
        path.display()
    );
}

fn set_secret(key: String, value: String) {
    let mut store = load_store().expect("Secret store not initialized. Run 'secret init' first");
    let password = ensure_authenticated(&store);

    let encrypted_value = encrypt_secret(&value, &password);

    if let Some(pos) = store.secrets.iter().position(|s| s.key == key) {
        store.secrets[pos].encrypted_value = encrypted_value;
        println!("{} Secret '{}' updated", "✓".green().bold(), key.yellow());
    } else {
        let now = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        store.secrets.push(Secret {
            key: key.clone(),
            encrypted_value,
            created_at: now,
        });
        println!("{} Secret '{}' stored", "✓".green().bold(), key.yellow());
    }

    save_store(&store);
}

fn get_secret(key: String) {
    let store = load_store().expect("Secret store not initialized. Run 'secret init' first");
    let password = ensure_authenticated(&store);

    match find_secret_fuzzy(&store, &key) {
        Some(secret) => match decrypt_secret(&secret.encrypted_value, &password) {
            Ok(value) => println!("{}", value),
            Err(e) => {
                eprintln!("{} {}", "✗".red().bold(), e);
                std::process::exit(1);
            }
        },
        None => {
            eprintln!("{} Secret '{}' not found", "✗".red().bold(), key.yellow());
            std::process::exit(1);
        }
    }
}

fn copy_secret(key: String) {
    let store = load_store().expect("Secret store not initialized. Run 'secret init' first");
    let password = ensure_authenticated(&store);

    let secret = match find_secret_fuzzy(&store, &key) {
        Some(s) => s,
        None => {
            eprintln!("{} Secret '{}' not found", "✗".red().bold(), key.yellow());
            std::process::exit(1);
        }
    };

    let value = match decrypt_secret(&secret.encrypted_value, &password) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{} {}", "✗".red().bold(), e);
            std::process::exit(1);
        }
    };

    copy_to_clipboard(&value);

    println!(
        "{} Secret '{}' copied to clipboard",
        "✓".green().bold(),
        secret.key.yellow()
    );
}

#[cfg(target_os = "linux")]
fn copy_to_clipboard(value: &str) {
    use std::process::{Command, Stdio};
    let mut child = Command::new("xclip")
        .arg("-selection")
        .arg("clipboard")
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to copy. Install xclip");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(value.as_bytes());
    }
}

#[cfg(target_os = "macos")]
fn copy_to_clipboard(value: &str) {
    use std::process::{Command, Stdio};
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to copy");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(value.as_bytes());
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn copy_to_clipboard(_value: &str) {
    eprintln!("{} Clipboard copy not supported on this platform", "✗".red().bold());
    std::process::exit(1);
}

fn list_secrets() {
    let store = load_store().expect("Secret store not initialized. Run 'secret init' first");
    ensure_authenticated(&store);

    if store.secrets.is_empty() {
        println!("{} No secrets stored yet", "→".cyan().bold());
        return;
    }

    println!("{} Stored secrets:\n", "→".cyan().bold());

    for (i, secret) in store.secrets.iter().enumerate() {
        println!(
            "  {}  {}",
            format!("{}", i + 1).yellow().bold(),
            secret.key.bright_cyan()
        );
    }

    println!("\n{} Total: {}", "•".dimmed(), store.secrets.len());
}

fn delete_secret(key: String) {
    let mut store = load_store().expect("Secret store not initialized. Run 'secret init' first");
    ensure_authenticated(&store);

    if let Some(pos) = store.secrets.iter().position(|s| s.key == key) {
        store.secrets.remove(pos);
        println!("{} Secret '{}' deleted", "✓".red().bold(), key.yellow());
        save_store(&store);
    } else {
        eprintln!("{} Secret '{}' not found", "✗".red().bold(), key.yellow());
        std::process::exit(1);
    }
}

fn change_password() {
    let mut store = load_store().expect("Secret store not initialized. Run 'secret init' first");
    let old_password = ensure_authenticated(&store);

    let new_password = prompt_password("Enter new password: ");
    let confirm_password = prompt_password("Confirm new password: ");

    if new_password != confirm_password {
        eprintln!("{} Passwords don't match", "✗".red().bold());
        std::process::exit(1);
    }

    for secret in &mut store.secrets {
        match decrypt_secret(&secret.encrypted_value, &old_password) {
            Ok(value) => {
                secret.encrypted_value = encrypt_secret(&value, &new_password);
            }
            Err(e) => {
                eprintln!("{} Failed to decrypt secret: {}", "✗".red().bold(), e);
                std::process::exit(1);
            }
        }
    }

    let salt = SaltString::generate(rand::thread_rng());
    store.password_hash = hash_password(&new_password, &salt);
    store.salt = salt.to_string();

    save_store(&store);
    println!("{} Password changed", "✓".green().bold());
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Set { key, value } => set_secret(key, value),
        Commands::Get { key } => get_secret(key),
        Commands::Copy { key } => copy_secret(key),
        Commands::List => list_secrets(),
        Commands::Delete { key } => delete_secret(key),
        Commands::Init => init_store(),
        Commands::ChangePassword => change_password(),
    }
}