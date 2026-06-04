pub const DEFAULT_IGNORE_CONTENT: &str = r#"# --- SYSTEM AND HIDDEN FILES ---
.git
.gitignore
.mrgignore
.DS_Store
Thumbs.db
desktop.ini

# --- CONFIDENTIAL INFORMATION (Secrets) ---
.env
.env.local
.env.development.local
.env.test.local
.env.production.local
*.pem
*.key
*.pub
id_rsa
id_ed25519
secrets.yaml
auth.json
credentials.json
*.pfx
*.p12

# --- DEPENDENCY AND BUILD FOLDERS (Heavy/Build) ---
node_modules
bower_components
jspm_packages
venv
.venv
env
__pycache__
target
build
dist
out
release_files
Debug
Release
.gradle
ipch
.terraform

# --- IDE and Development Tools ---
.idea
.vscode
*.swp
*.swo
.eslintcache
.sass-cache
.cache

# --- BINARY FILES AND COMPILATION ---
*.pyc
*.pyo
*.pyd
*.o
*.obj
*.so
*.dll
*.dylib
*.class
*.jar
*.exe
*.bin
*.exp
*.lib
*.def
*.out
core

# --- ARCHIVES AND COMPRESSED DATA ---
*.zip
*.tar
*.gz
*.7z
*.rar
*.dmg
*.iso
*.apk
*.ipa

# --- DATABASES AND DATA (Non-text) ---
*.db
*.sqlite
*.sqlite3
*.pickle
*.pkl
*.h5
*.npy
*.parquet

# --- MEDIA AND DOCUMENTS (Binary) ---
*.png
*.jpg
*.jpeg
*.gif
*.ico
*.pdf
*.mp4
*.avi
*.mp3
*.ttf
*.otf
*.woff
*.woff2
*.eot

# --- LOGS AND INFORMATION NOISE ---
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*
*.bak
*.tmp
*.temp
*.stackdump
mrg-*.txt

# --- CONFIGURATIONS THAT MAY BE UNNECESSARY ---
Cargo.lock
*.xml
package-lock.json
"#;
