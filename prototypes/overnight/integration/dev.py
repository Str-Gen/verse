#!/usr/bin/env python3
"""Build and run checkout-local readers without compiling during launch."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys

HERE = Path(__file__).resolve().parent
COMPONENTS = HERE.parent
BINARIES = {
    "core": (COMPONENTS / "core", "reader-core"),
    "qt": (COMPONENTS / "qt", "overnight-qt-reader"),
    "tauri": (COMPONENTS / "tauri/src-tauri", "quiet-reader-tauri"),
}


def binary(component, profile):
    directory, name = BINARIES[component]
    return directory / "target" / profile / name


def commands(name, frontend):
    result = {}
    if frontend in ("both", "qt"):
        result[name] = "qt"
        result[f"{name}-qt"] = "qt"
    if frontend in ("both", "tauri"):
        result[f"{name}-tauri"] = "tauri"
    return result


def check_links(bin_dir, names):
    for name in names:
        link = bin_dir / name
        target = HERE / ".local/commands" / name
        if os.path.lexists(link) and not (link.is_symlink() and link.resolve() == target):
            raise ValueError(f"Refusing to replace unrelated command: {link}")


def build(args):
    if not re.fullmatch(r"[a-z][a-z0-9-]*", args.name):
        raise ValueError("Command names must start with a lowercase letter and contain only a-z, 0-9 or -.")
    bin_dir = args.bin_dir.expanduser().resolve()
    names = commands(args.name, args.frontend)
    if not args.no_link:
        check_links(bin_dir, names)
    profile = "debug" if args.debug else "release"
    for component in ["core", *dict.fromkeys(names.values())]:
        directory, _ = BINARIES[component]
        command = ["cargo", "+1.99.0", "build", "--locked", "-j", "2",
                   "--manifest-path", str(directory / "Cargo.toml"),
                   "--target-dir", str(directory / "target")]
        if profile == "release":
            command.append("--release")
        subprocess.run(command, cwd=directory, check=True)
    if args.no_link:
        return
    # Recheck after compiling; never overwrite a command created in the meantime.
    check_links(bin_dir, names)
    bin_dir.mkdir(parents=True, exist_ok=True)
    wrapper_dir = HERE / ".local/commands"
    wrapper_dir.mkdir(parents=True, exist_ok=True)
    for name, frontend in names.items():
        wrapper = wrapper_dir / name
        body = "#!/bin/sh\nexec " + shlex.join([
            sys.executable, str(Path(__file__).resolve()), "launch", frontend, profile, name, "--"
        ]) + ' "$@"\n'
        temporary = wrapper.with_name(f".{name}.{os.getpid()}.tmp")
        try:
            temporary.write_text(body)
            temporary.chmod(0o755)
            temporary.replace(wrapper)
        finally:
            temporary.unlink(missing_ok=True)
        link = bin_dir / name
        if not link.is_symlink():
            link.symlink_to(wrapper)
        print(f"{link} -> {frontend} ({profile})", flush=True)
    if str(bin_dir) not in os.environ.get("PATH", "").split(os.pathsep):
        print(f"Add this directory to your shell PATH: {bin_dir}")
    print(f"Ready: {args.name if 'qt' in names.values() else args.name + '-tauri'} book.epub")


def launch(frontend, profile, name, argv):
    parser = argparse.ArgumentParser(prog=name, description=f"Verse {frontend} reader, using the existing {profile} build.")
    parser.add_argument("book", nargs="?", type=Path, help="EPUB path, relative to the current directory or absolute")
    inputs = parser.add_mutually_exclusive_group()
    inputs.add_argument("--epub", type=Path, help="explicit EPUB path")
    inputs.add_argument("--book-json", type=Path, help="already normalized book JSON")
    inputs.add_argument("--demo", action="store_true", help="open the synthetic demo EPUB")
    parser.add_argument("--state-dir", type=Path, help="override this frontend's state directory")
    if frontend == "tauri":
        parser.add_argument("--self-test", action="store_true", help="run the native synthetic checks")
    parser.add_argument("--version", action="version", version=f"Verse development build ({frontend}, {profile})")
    args = parser.parse_args(argv)
    if args.book is not None and (args.epub is not None or args.book_json is not None or args.demo):
        parser.error("provide one book input")
    executable = binary(frontend, profile)
    importer = binary("core", profile)
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError(f"Build missing: {executable}. Run ./build.sh again in the checkout.")
    epub = args.book or args.epub
    if args.demo:
        epub = COMPONENTS / "core/fixtures/complex.epub"
    book_json = args.book_json.resolve(strict=True) if args.book_json else None
    if epub is not None:
        epub = epub.resolve(strict=True)
        if not importer.is_file() or not os.access(importer, os.X_OK):
            raise ValueError("Built EPUB importer missing. Run ./build.sh again in the checkout.")
        cache = HERE / ".local"
        cache.mkdir(parents=True, exist_ok=True)
        # Match the existing launcher's canonical-path cache and identity store.
        key = hashlib.sha256(os.fsencode(epub)).hexdigest()
        book_json = cache / f"{key}.json"
        subprocess.run([str(importer), "export", "--epub", str(epub), "--output", str(book_json),
                        "--store", str(cache / "identities")], check=True)
    command = [str(executable)]
    if book_json is not None:
        command.extend(["--book-json", str(book_json)])
    environment = os.environ.copy()
    if args.state_dir:
        directory = str(args.state_dir.expanduser().resolve())
        environment["READER_STATE_DIR" if frontend == "qt" else "READER_SCRATCH"] = directory
    if frontend == "tauri":
        if sys.platform.startswith("linux"):
            environment.setdefault("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        if args.self_test:
            command.append("--self-test")
    os.execve(executable, command, environment)


def main():
    if len(sys.argv) >= 5 and sys.argv[1] == "launch":
        frontend, profile, name = sys.argv[2:5]
        if frontend not in ("qt", "tauri") or profile not in ("debug", "release"):
            raise ValueError("Invalid launcher configuration")
        argv = sys.argv[5:]
        launch(frontend, profile, name, argv[1:] if argv[:1] == ["--"] else argv)
        return
    parser = argparse.ArgumentParser(description="Build Verse and link its checkout-local commands onto PATH.")
    parser.add_argument("action", choices=["build"])
    parser.add_argument("--debug", action="store_true", help="build debug binaries instead of release")
    parser.add_argument("--frontend", choices=["both", "qt", "tauri"], default="both")
    parser.add_argument("--name", default="verse", help="base command name (default: verse)")
    parser.add_argument("--bin-dir", type=Path, default=Path.home() / ".local/bin")
    parser.add_argument("--no-link", action="store_true", help="build without creating PATH commands")
    build(parser.parse_args())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Verse: {error}", file=sys.stderr)
        sys.exit(1)
