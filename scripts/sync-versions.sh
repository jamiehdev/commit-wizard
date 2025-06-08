#!/bin/bash

# sync-versions.sh - Synchronize versions across all packages
# Usage: ./scripts/sync-versions.sh [version]

set -e

NEW_VERSION="${1:-}"

if [ -z "$NEW_VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 1.1.0"
    exit 1
fi

# Validate version format (basic semver check)
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9\.-]+)?$'; then
    echo "Error: Invalid version format. Use semantic versioning (e.g., 1.0.0)"
    exit 1
fi

echo "🔄 Syncing all packages to version $NEW_VERSION..."

# Update workspace version
echo "📝 Updating workspace Cargo.toml..."
sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Update main package.json version
echo "📝 Updating main package.json..."
sed -i.bak "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" commit-wizard-napi/package.json

# Update optional dependencies in main package.json
echo "📝 Updating optional dependencies in main package.json..."
sed -i.bak "s/\"@jamiehdev\/commit-wizard-linux-x64-gnu\": \"[^\"]*\"/\"@jamiehdev\/commit-wizard-linux-x64-gnu\": \"$NEW_VERSION\"/g" commit-wizard-napi/package.json
sed -i.bak "s/\"@jamiehdev\/commit-wizard-darwin-x64\": \"[^\"]*\"/\"@jamiehdev\/commit-wizard-darwin-x64\": \"$NEW_VERSION\"/g" commit-wizard-napi/package.json
sed -i.bak "s/\"@jamiehdev\/commit-wizard-darwin-arm64\": \"[^\"]*\"/\"@jamiehdev\/commit-wizard-darwin-arm64\": \"$NEW_VERSION\"/g" commit-wizard-napi/package.json
sed -i.bak "s/\"@jamiehdev\/commit-wizard-win32-x64-msvc\": \"[^\"]*\"/\"@jamiehdev\/commit-wizard-win32-x64-msvc\": \"$NEW_VERSION\"/g" commit-wizard-napi/package.json

# Update npm platform-specific package.json files
echo "📝 Updating platform-specific package.json files..."
for platform_dir in commit-wizard-napi/npm/*; do
    if [ -d "$platform_dir" ]; then
        platform_name=$(basename "$platform_dir")
        package_json="$platform_dir/package.json"
        if [ -f "$package_json" ]; then
            echo "  📝 Updating $package_json..."
            sed -i.bak "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" "$package_json"
        fi
    fi
done

# Clean up backup files
find . -name "*.bak" -delete

echo "✅ All versions synced to $NEW_VERSION"

# Verify the changes
echo "🔍 Verification:"
echo "Workspace version: $(grep '^version = ' Cargo.toml)"
echo "Package.json version: $(grep '"version":' commit-wizard-napi/package.json)"

# Check if cargo check passes
echo "🧪 Running cargo check..."
if cargo check; then
    echo "✅ Cargo check passed"
else
    echo "❌ Cargo check failed - please review the changes"
    exit 1
fi

echo "🎉 Version sync complete! Remember to:"
echo "  1. Review the changes: git diff"
echo "  2. Test the build: cargo build --release"
echo "  3. Commit the changes: git add . && git commit -m \"chore: bump version to $NEW_VERSION\""
echo "  4. Tag the release: git tag v$NEW_VERSION"