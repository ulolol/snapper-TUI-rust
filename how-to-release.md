# How to Release `snapper‑TUI‑rust`

This guide explains the minimal steps to publish a new version of the project using the GitHub Actions workflow we set up.

---

## 1️⃣ Prepare the release

1. **Update the version** in `Cargo.toml` (e.g. `0.2.0`).
2. Commit the change:
   ```bash
   git add Cargo.toml
   git commit -m "Bump version to 0.2.0"
   ```

## 2️⃣ Create a GitHub PAT (if you haven’t already)

| Scope needed | Reason |
|--------------|--------|
| **Contents – Read & Write** | Allows the workflow to create a GitHub Release and upload assets. |

1. Generate a classic PAT with the **Contents** permission.
2. In the repository go to **Settings → Secrets → Actions → New repository secret**.
3. Name it `RELEASE_PAT` and paste the PAT value.

## 3️⃣ Tag the new version

The workflow triggers releases only on tags that match `v*`.
```bash
git tag -s v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```
> Replace `0.2.0` with the version you set in `Cargo.toml`.

## 4️⃣ What the workflow does

- **Build** (`cargo build --release`) and **test** (`cargo test`).
- If the tag push succeeds, `softprops/action-gh-release@v2` creates a release named `Release vX.Y.Z`.
- The compiled binary `target/release/snapper‑TUI‑rust` is uploaded as an asset.
- The workflow uses the `RELEASE_PAT` secret, so the default `GITHUB_TOKEN` permissions are not required for releases.

## 5️⃣ Verify the release

1. Open **GitHub → Releases** for the repository.
2. You should see a new entry titled `Release vX.Y.Z` with the binary attached.
3. Download the asset to confirm it works.

---

**Tip:** For subsequent releases, just repeat steps 1 → 3. The CI will handle the rest automatically.
