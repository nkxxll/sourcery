#!/usr/bin/env bash
# Creates a small git repo under test_fixtures/ with a few commits for integration testing.
# If the repo already exists, resets it to the tip of main.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)/test_fixtures/test_repo"

CHECKOUT_DIR="${REPO_DIR}_checkout"

reset_repo() {
    cd "$1"
    git checkout main --force
    git clean -fd
}

if [ -d "$REPO_DIR/.git" ]; then
    reset_repo "$REPO_DIR"
    reset_repo "$CHECKOUT_DIR"
    echo "Test repos reset"
    exit 0
fi

rm -rf "$REPO_DIR"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"

git init
git config user.email "test@example.com"
git config user.name "Test Author"

# Commit 1: initial file
cat > hello.rs <<'EOF'
fn main() {
    println!("Hello, world!");
}
EOF
git add hello.rs
git commit -m "Initial commit"

# Commit 2: add a second file
mkdir -p src
cat > src/lib.rs <<'EOF'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
EOF
git add src/lib.rs
git commit -m "Add lib with add function"

# Commit 3: modify hello.rs
cat > hello.rs <<'EOF'
fn main() {
    let result = add(1, 2);
    println!("1 + 2 = {}", result);
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
EOF
git add hello.rs
git commit -m "Use add function in main"

echo "Test repo created at $REPO_DIR with $(git rev-list --count HEAD) commits."

# Create a copy for checkout tests
cp -R "$REPO_DIR" "$CHECKOUT_DIR"
echo "Checkout copy created at $CHECKOUT_DIR"
