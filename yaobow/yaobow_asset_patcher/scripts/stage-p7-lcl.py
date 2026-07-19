#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile
import urllib.request


CRATE_DIR = Path(__file__).resolve().parent.parent
MANIFEST_PATH = CRATE_DIR / "p7-lcl-release.json"


def parse_args():
    parser = argparse.ArgumentParser(
        description="Verify and stage the pinned p7-lcl release resources."
    )
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--archive", type=Path)
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=CRATE_DIR / "target" / "p7-lcl-cache",
    )
    return parser.parse_args()


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def download(url, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        dir=destination.parent, prefix=destination.name + ".", delete=False
    ) as temporary:
        temporary_path = Path(temporary.name)
        try:
            with urllib.request.urlopen(url) as response:
                shutil.copyfileobj(response, temporary)
            os.replace(temporary_path, destination)
        except BaseException:
            temporary_path.unlink(missing_ok=True)
            raise


def validate_members(archive, native_library):
    expected = {
        "LICENSE",
        "THIRD_PARTY_NOTICES.md",
        "native/lib/" + native_library,
        "p7.toml",
        "src/mod.p7",
    }
    members = {}
    for member in archive.getmembers():
        path = PurePosixPath(member.name)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError(f"unsafe archive path: {member.name}")
        if not member.isfile():
            raise ValueError(f"unexpected non-file archive member: {member.name}")
        if member.name in members:
            raise ValueError(f"duplicate archive member: {member.name}")
        members[member.name] = member
    actual = set(members)
    if actual != expected:
        missing = sorted(expected - actual)
        unexpected = sorted(actual - expected)
        raise ValueError(
            f"unexpected archive layout; missing={missing}, unexpected={unexpected}"
        )
    return members


def extract_member(archive, member, destination):
    source = archive.extractfile(member)
    if source is None:
        raise ValueError(f"failed to read archive member: {member.name}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("wb") as output:
        shutil.copyfileobj(source, output)
    destination.chmod(member.mode & 0o777)


def stage(archive_path, output, native_library):
    destination = output / "p7-lcl"
    output.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix="p7-lcl.", dir=output))
    try:
        with tarfile.open(archive_path, "r:gz") as archive:
            members = validate_members(archive, native_library)
            for name in (
                "LICENSE",
                "THIRD_PARTY_NOTICES.md",
                "src/mod.p7",
                "native/lib/" + native_library,
            ):
                extract_member(archive, members[name], temporary / name)
        if destination.exists():
            shutil.rmtree(destination)
        os.replace(temporary, destination)
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def main():
    args = parse_args()
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    try:
        target = manifest["targets"][args.target]
    except KeyError:
        supported = ", ".join(sorted(manifest["targets"]))
        raise SystemExit(f"unsupported target {args.target!r}; expected one of: {supported}")

    if args.archive is not None:
        archive_path = args.archive
    else:
        archive_name = target["url"].rsplit("/", 1)[-1]
        archive_path = args.cache_dir / archive_name
        if not archive_path.exists():
            download(target["url"], archive_path)

    actual_hash = sha256(archive_path)
    if actual_hash != target["sha256"]:
        raise SystemExit(
            f"SHA-256 mismatch for {archive_path}: "
            f"expected {target['sha256']}, got {actual_hash}"
        )

    stage(archive_path, args.output, target["native_library"])
    print(args.output / "p7-lcl")


if __name__ == "__main__":
    main()
