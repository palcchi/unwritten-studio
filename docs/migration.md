# Compatibility and migration

Unwritten Studio uses new technical package names, app identifiers and file extensions. Existing
projects and user data remain readable where a safe compatibility path exists.

## Projects and templates

- New JSON projects use `.uwproj`; new XML copies use `.uwprojx`.
- Existing `.ecproj` and `.ecprojx` projects can still be opened. Saving a new copy uses the new
  extensions. Previous XML root tags are normalized on import.
- New user templates use `.uwtemplate`. Existing `.ectemplate` archives are listed and imported,
  including archives containing the prior `project.ecproj` member and format identifier.

## Settings and browser data

- On Linux, if the new settings directory does not exist, the first launch copies the prior
  `effectcraft` directory under `$XDG_CONFIG_HOME` (or `~/.config`) to `unwritten-studio`. It never
  removes or overwrites the original directory; if the destination already exists, no merge is
  attempted.
- `UNWRITTEN_STUDIO_CONFIG_DIR` takes precedence. The former `EFFECTCRAFT_CONFIG_DIR` remains a
  fallback alias for existing launch scripts. The new control-port variable also takes precedence
  over the former `EFFECTCRAFT_CONTROL_PORT` alias.
- Browser OPFS and IndexedDB keep their existing internal storage namespace so saved projects,
  media, settings and session recovery are not orphaned by the rename.
- The browser API is now `window.unwrittenStudio`; `window.effectcraft` remains an alias for
  existing automation.

## Technical names

The desktop executable and CLI are now `unwritten-studio` and `unwritten-studio-cli`. New Rust
crate names, package IDs, installer metadata, icons and asset paths use Unwritten Studio. Linux
package metadata uses `com.unwrittenstudio.app`.
