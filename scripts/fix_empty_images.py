#!/usr/bin/env python3
"""
Script to find and fix empty/missing image files in the catalog.

Usage:
    python fix_empty_images.py <catalog_db_path> <media_path> <download_url_template>

The download_url_template should contain {image_id} as a placeholder.
Example: https://example.com/images/{image_id}
"""

import argparse
import os
import sqlite3
import sys
from urllib.request import urlopen, Request
from urllib.error import URLError, HTTPError


def get_all_images(db_path: str) -> list[tuple[str, str]]:
    """Get all image IDs and URIs from the catalog database."""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute("SELECT id, uri FROM images")
    images = cursor.fetchall()
    conn.close()
    return images


def check_image_file(media_path: str, uri: str) -> tuple[bool, str]:
    """
    Check if an image file exists and is not empty.
    Returns (is_valid, reason).
    """
    full_path = os.path.join(media_path, uri)

    if not os.path.exists(full_path):
        return False, "file_missing"

    if os.path.getsize(full_path) == 0:
        return False, "file_empty"

    return True, "ok"


def download_image(url: str, dest_path: str) -> bool:
    """Download an image from URL and save to dest_path."""
    try:
        # Ensure parent directory exists
        os.makedirs(os.path.dirname(dest_path), exist_ok=True)

        request = Request(url, headers={"User-Agent": "PezzottifyImageFixer/1.0"})
        with urlopen(request, timeout=30) as response:
            content = response.read()

            if len(content) == 0:
                print(f"  Warning: Downloaded empty content from {url}")
                return False

            with open(dest_path, "wb") as f:
                f.write(content)

            return True
    except HTTPError as e:
        print(f"  HTTP Error {e.code}: {e.reason}")
        return False
    except URLError as e:
        print(f"  URL Error: {e.reason}")
        return False
    except Exception as e:
        print(f"  Error: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(
        description="Find and fix empty/missing image files in the catalog."
    )
    parser.add_argument("catalog_db", help="Path to the catalog SQLite database")
    parser.add_argument("media_path", help="Path to the media directory")
    parser.add_argument(
        "url_template",
        help="URL template with {image_id} placeholder (e.g., https://example.com/images/{image_id})",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Only report missing/empty images without downloading",
    )

    args = parser.parse_args()

    # Validate paths
    if not os.path.exists(args.catalog_db):
        print(f"Error: Catalog database not found: {args.catalog_db}")
        sys.exit(1)

    if not os.path.isdir(args.media_path):
        print(f"Error: Media path is not a directory: {args.media_path}")
        sys.exit(1)

    if "{image_id}" not in args.url_template:
        print("Error: URL template must contain {image_id} placeholder")
        sys.exit(1)

    # Get all images from database
    print(f"Reading images from {args.catalog_db}...")
    images = get_all_images(args.catalog_db)
    print(f"Found {len(images)} images in database")

    # Check each image
    missing = []
    empty = []
    valid = 0

    print("Checking image files...")
    for image_id, uri in images:
        is_valid, reason = check_image_file(args.media_path, uri)
        if is_valid:
            valid += 1
        elif reason == "file_missing":
            missing.append((image_id, uri))
        elif reason == "file_empty":
            empty.append((image_id, uri))

    print(f"\nResults:")
    print(f"  Valid: {valid}")
    print(f"  Missing: {len(missing)}")
    print(f"  Empty: {len(empty)}")

    problematic = missing + empty
    if not problematic:
        print("\nAll images are valid!")
        return

    if args.dry_run:
        print("\n[DRY RUN] Would download the following images:")
        for image_id, uri in problematic:
            print(f"  {image_id}: {uri}")
        return

    # Download missing/empty images
    print(f"\nDownloading {len(problematic)} images...")
    success = 0
    failed = 0

    for i, (image_id, uri) in enumerate(problematic, 1):
        dest_path = os.path.join(args.media_path, uri)
        url = args.url_template.replace("{image_id}", image_id)

        print(f"[{i}/{len(problematic)}] Downloading {image_id}...")

        if download_image(url, dest_path):
            success += 1
            print(f"  Saved to {dest_path}")
        else:
            failed += 1

    print(f"\nDownload complete:")
    print(f"  Success: {success}")
    print(f"  Failed: {failed}")


if __name__ == "__main__":
    main()
