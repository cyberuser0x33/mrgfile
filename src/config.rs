pub const DEFAULT_IGNORE_CONTENT: &str = r#"# === SYSTEM AND HIDDEN FILES ===
.git/
.gitignore
.github/
.gitattributes
.mrgignore
.DS_Store
Thumbs.db
desktop.ini
.Trashes
.fseventsd

# === CONFIDENTIAL INFORMATION (Secrets) ===
.env*
*.pem
*.key
*.pub
*.crt
*.pfx
*.p12
id_rsa
id_ed25519
secrets.*
auth.json
credentials.json
*.keystore
service-account.json

# === DEPENDENCY AND BUILD FOLDERS ===
node_modules/
bower_components/
jspm_packages/
venv/
.venv/
env/
__pycache__/
target/
build/
dist/
out/
.next/
.nuxt/
.turbo/
.cache/
bin/
obj/
release/

# === IDE AND DEV TOOLS ===
.idea/
.vscode/
*.swp
*.swo
.eslintcache
.sass-cache
.parcel-cache
.terraform/
.pytest_cache/
.mypy_cache/
.ruff_cache/
.tox/

# === MOBILE AND SPECIFIC ECOSYSTEMS ===
ios/Pods/
android/.gradle/
.expo/
fastlane/
.flutter-plugins
.pub-cache/

# === BINARY AND LARGE FILES ===
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
*.out
*.app
*.ipa
*.apk

# === ARCHIVES AND MEDIA ===
*.zip
*.tar*
*.gz
*.7z
*.rar
*.dmg
*.iso
*.png
*.jpg
*.jpeg
*.gif
*.ico
*.pdf
*.mp4
*.mp3
*.ttf
*.woff*

# === LOGS AND NOISE ===
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*
*.bak
*.tmp
*.temp
*.stackdump
mrg-*.txt

# === LOCK FILES ===
package-lock.json
yarn.lock
pnpm-lock.yaml
Cargo.lock
poetry.lock
composer.lock

# ========== Custom User Patterns =========

"#;