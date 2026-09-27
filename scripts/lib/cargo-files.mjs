// Small, line-based readers/writers for the two Cargo TOML files whose
// version has to stay in sync with everything else (see set-version.mjs /
// check-version.mjs). Deliberately not a general TOML parser: scoped
// line-by-line scanning is easier to reason about correct than a clever
// regex that has to skip past an unrelated section, and it round-trips
// each line's original ending (LF or CRLF) untouched.

/** Splits text into lines, each one keeping its own trailing line ending. */
function splitLines(text) {
  return text.split(/(?<=\n)/);
}

function versionLineMatch(line) {
  // Each "line" here already includes its own trailing line ending (LF or
  // CRLF, see splitLines above); the `s` flag lets the trailing `(.*)`
  // capture group swallow that ending too, since `.` otherwise never
  // matches \r or \n.
  return line.match(/^(\s*version\s*=\s*)"([^"]*)"(.*)$/s);
}

/**
 * Returns [startIndex, endIndex) of the line range for the first
 * `[section]` (or `[[section]]`) block whose header line equals `header`
 * exactly, running from just after the header to the next line that opens
 * any section (`[...]`), or end of file.
 */
function findSectionRange(lines, header) {
  const start = lines.findIndex((l) => l.trim() === header);
  if (start === -1) return null;
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    if (/^\[.*\]\s*$/.test(lines[i].trim())) {
      end = i;
      break;
    }
  }
  return [start, end];
}

/**
 * Returns [startIndex, endIndex) for the `[[package]]` block in a Cargo.lock
 * whose body contains a `name = "<name>"` line, running from that block's
 * `[[package]]` header to the next `[[package]]` header or end of file.
 */
function findLockPackageRange(lines, name) {
  const headerLines = [];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() === "[[package]]") headerLines.push(i);
  }
  for (let i = 0; i < headerLines.length; i++) {
    const start = headerLines[i];
    const end = i + 1 < headerLines.length ? headerLines[i + 1] : lines.length;
    const isMatch = lines.slice(start, end).some((l) => l.trim() === `name = "${name}"`);
    if (isMatch) return [start, end];
  }
  return null;
}

function readVersionInRange(lines, range, label) {
  if (!range) throw new Error(`${label}: section not found`);
  const [start, end] = range;
  for (let i = start; i < end; i++) {
    const m = versionLineMatch(lines[i]);
    if (m) return m[2];
  }
  throw new Error(`${label}: no version line found in section`);
}

function writeVersionInRange(lines, range, label, newVersion) {
  if (!range) throw new Error(`${label}: section not found`);
  const [start, end] = range;
  for (let i = start; i < end; i++) {
    const m = versionLineMatch(lines[i]);
    if (m) {
      lines[i] = `${m[1]}"${newVersion}"${m[3]}`;
      return;
    }
  }
  throw new Error(`${label}: no version line found in section`);
}

export function getCargoTomlVersion(text) {
  const lines = splitLines(text);
  return readVersionInRange(lines, findSectionRange(lines, "[package]"), "Cargo.toml [package]");
}

export function setCargoTomlVersion(text, newVersion) {
  const lines = splitLines(text);
  writeVersionInRange(lines, findSectionRange(lines, "[package]"), "Cargo.toml [package]", newVersion);
  return lines.join("");
}

export function getCargoLockPackageVersion(text, name) {
  const lines = splitLines(text);
  return readVersionInRange(lines, findLockPackageRange(lines, name), `Cargo.lock [[package]] "${name}"`);
}

export function setCargoLockPackageVersion(text, name, newVersion) {
  const lines = splitLines(text);
  writeVersionInRange(lines, findLockPackageRange(lines, name), `Cargo.lock [[package]] "${name}"`, newVersion);
  return lines.join("");
}
