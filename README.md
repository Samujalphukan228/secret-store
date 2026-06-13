

<p align="center">
  <img src="https://res.cloudinary.com/dzulab559/image/upload/v1781341183/secret_1_wckq5b.png" alt="secret-store logo" width="700">
</p>


# secret-store

Encrypted CLI secret manager for developers. Store API keys, tokens, passwords locally with a master password. Built-in fuzzy search.

```bash
ss binance-api "sk_live_xxx"
sg b                          # finds binance-api instantly
sc b                          # copies to clipboard
sl                            # list all (encrypted)
```

## Features

- **Encrypted storage** — ChaCha20-Poly1305 encryption
- **Master password** — Argon2 hashing
- **Fuzzy search** — `sg b` finds `binance-api`
- **Initialism matching** — `sg ba` finds `binance-api`
- **Short aliases** — `ss`, `sg`, `sc`, `sl`, `sd`, `cp`
- **Clipboard copy** — auto-copy secrets to clipboard
- **File-based** — `~/.secret-store/secrets.json` (0600 permissions)

## Install

```bash
. <(curl -sSf https://raw.githubusercontent.com/Samujalphukan228/secret-store/master/install.sh)
```

This installs Rust (if needed), builds the binary, adds `~/.local/bin` to your `PATH`, and is ready to use immediately — no extra steps.

### Manual install

```bash
git clone https://github.com/Samujalphukan228/secret-store
cd secret-store
cargo build --release
cp target/release/secret ~/.local/bin/secret
```

## Usage

### Initialize

```bash
secret init
```

Sets up `~/.secret-store/secrets.json` and prompts for master password.

### Store a secret

```bash
secret set binance-api "sk_live_xxx"
ss github-token "ghp_yyy"
ss openai-key "sk-zzz"
```

### Get a secret

```bash
secret get binance-api
sg b                    # fuzzy search finds binance-api
sg gi                   # fuzzy search finds github-token
```

### Copy to clipboard

```bash
secret copy binance-api
sc ba
```

Copies the decrypted secret to your clipboard (Linux: xclip, macOS: pbcopy).

### List all secrets

```bash
secret list
sl
```

Shows all stored keys (masked, encrypted).

### Delete a secret

```bash
secret delete old-key
sd old-key
```

### Change master password

```bash
secret change-password
cp
```

Re-encrypts all secrets with new password.

## Aliases

| Long | Short | Command |
|------|-------|---------|
| `set` | `ss` | Store secret |
| `get` | `sg` | Retrieve secret |
| `copy` | `sc` | Copy to clipboard |
| `list` | `sl` | List all keys |
| `delete` | `sd` | Delete secret |
| `change-password` | `cp` | Change password |

## Fuzzy Search

Works with:

- **Exact match** — `sg binance-api` → `binance-api`
- **Contains** — `sg binance` → `binance-api`
- **Initials** — `sg ba` → `binance-api`
- **Subsequence** — `sg bapi` → `binance-api`

## Security

- Secrets encrypted with **ChaCha20-Poly1305**
- Master password hashed with **Argon2**
- File permissions: **0600** (readable only by you)
- Never logged or printed except on explicit `get`
- No network — everything local

## Requirements

- **Linux**: `xclip` for clipboard (`sudo pacman -S xclip` / `sudo apt install xclip`)
- **macOS**: `pbcopy` (built-in)

## License

MIT